use crate::error::{OutlookError, OutlookResult, ScanWarning};
use crate::nk2::query_provider_to_json;
use crate::regf::{RegKeyNode, RegValue, RegfHive};
use guap_mcp::{ActionCommand, ActionRiskLevel, McpToolError};
use guap_table::{
    CellValue, ColumnDef, ColumnType, Row, TableData, TableError, TableProvider, TableQuery, TableSchema, apply_query,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HiveRoot {
    Hkcu,
    Hklm,
    HklmWow64,
}

impl HiveRoot {
    pub fn label(self) -> &'static str {
        match self {
            HiveRoot::Hkcu => "HKCU",
            HiveRoot::Hklm => "HKLM (64-bit)",
            HiveRoot::HklmWow64 => "HKLM (WOW6432Node)",
        }
    }
}

/// Safe abstraction over live Windows Registry (`winreg`), offline `regf` hives, and deterministic mock trees.
pub trait RegistryBackend: Send + Sync {
    fn enum_subkeys(&self, root: HiveRoot, path: &str) -> OutlookResult<Vec<String>>;
    fn read_values(&self, root: HiveRoot, path: &str) -> OutlookResult<BTreeMap<String, RegValue>>;
    fn set_dword(&self, root: HiveRoot, path: &str, value_name: &str, val: u32) -> OutlookResult<()>;
}

/// Live Windows Registry backend implemented via safe `winreg = "0.56"` (zero `unsafe`).
#[cfg(windows)]
pub struct LiveWindowsRegistry;

#[cfg(windows)]
impl LiveWindowsRegistry {
    fn open_key_ro(&self, root: HiveRoot, path: &str) -> OutlookResult<winreg::RegKey> {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
        let (predef, flags) = match root {
            HiveRoot::Hkcu => (HKEY_CURRENT_USER, KEY_READ),
            HiveRoot::Hklm => (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_64KEY),
            HiveRoot::HklmWow64 => (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY),
        };
        let base = RegKey::predef(predef);
        base.open_subkey_with_flags(path, flags)
            .map_err(|e| map_reg_io_err(root, path, e))
    }

    fn open_key_rw(&self, root: HiveRoot, path: &str) -> OutlookResult<winreg::RegKey> {
        use winreg::RegKey;
        use winreg::enums::{
            HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, KEY_WRITE,
        };
        let (predef, flags) = match root {
            HiveRoot::Hkcu => (HKEY_CURRENT_USER, KEY_READ | KEY_WRITE),
            HiveRoot::Hklm => (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WRITE | KEY_WOW64_64KEY),
            HiveRoot::HklmWow64 => (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WRITE | KEY_WOW64_32KEY),
        };
        let base = RegKey::predef(predef);
        base.open_subkey_with_flags(path, flags)
            .map_err(|e| map_reg_io_err(root, path, e))
    }
}

#[cfg(windows)]
fn map_reg_io_err(root: HiveRoot, path: &str, err: std::io::Error) -> OutlookError {
    match err.kind() {
        std::io::ErrorKind::NotFound => OutlookError::NotFound(format!("{}\\{}", root.label(), path)),
        std::io::ErrorKind::PermissionDenied => OutlookError::AccessDenied {
            scope: format!("{}\\{}", root.label(), path),
            detail: err.to_string(),
        },
        _ => OutlookError::Io(err),
    }
}

#[cfg(windows)]
impl RegistryBackend for LiveWindowsRegistry {
    fn enum_subkeys(&self, root: HiveRoot, path: &str) -> OutlookResult<Vec<String>> {
        let key = self.open_key_ro(root, path)?;
        let mut names = Vec::new();
        for res in key.enum_keys() {
            match res {
                Ok(n) => names.push(n),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => continue,
                Err(e) => return Err(map_reg_io_err(root, path, e)),
            }
        }
        Ok(names)
    }

    fn read_values(&self, root: HiveRoot, path: &str) -> OutlookResult<BTreeMap<String, RegValue>> {
        use winreg::enums::RegType;
        let key = self.open_key_ro(root, path)?;
        let mut map = BTreeMap::new();
        for res in key.enum_values() {
            let Ok((name, reg_val)) = res else { continue };
            let converted = match reg_val.vtype {
                RegType::REG_SZ => RegValue::String(decode_reg_string(&reg_val.bytes)),
                RegType::REG_EXPAND_SZ => RegValue::ExpandString(decode_reg_string(&reg_val.bytes)),
                RegType::REG_DWORD if reg_val.bytes.len() >= 4 => {
                    RegValue::Dword(u32::from_le_bytes(reg_val.bytes[0..4].try_into().unwrap()))
                }
                RegType::REG_QWORD if reg_val.bytes.len() >= 8 => {
                    RegValue::Qword(u64::from_le_bytes(reg_val.bytes[0..8].try_into().unwrap()))
                }
                _ => RegValue::Binary(reg_val.bytes.to_vec()),
            };
            map.insert(name, converted);
        }
        Ok(map)
    }

    fn set_dword(&self, root: HiveRoot, path: &str, value_name: &str, val: u32) -> OutlookResult<()> {
        let key = self.open_key_rw(root, path)?;
        key.set_value(value_name, &val)
            .map_err(|e| map_reg_io_err(root, path, e))
    }
}

#[cfg(windows)]
fn decode_reg_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes.len().is_multiple_of(2) {
        crate::time::decode_utf16le(bytes)
    } else {
        crate::time::decode_cp1252(bytes)
    }
}

/// Offline `regf` hive backend supporting multiple `NTUSER.DAT`/`UsrClass.dat` (`HKCU`) and `SOFTWARE` (`HKLM`) hives.
pub struct OfflineRegfBackend {
    pub ntuser_hives: Vec<RegfHive>,
    pub software_hives: Vec<RegfHive>,
}

impl OfflineRegfBackend {
    pub fn from_hive_paths(paths: &[PathBuf]) -> OutlookResult<Self> {
        let mut ntuser_hives = Vec::new();
        let mut software_hives = Vec::new();
        for path in paths {
            let hive = RegfHive::from_file(path)?;
            let fname = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let has_office = hive.root.get_path("Software\\Microsoft\\Office").is_some();
            if fname.contains("ntuser") || fname.contains("usrclass") {
                ntuser_hives.push(hive);
            } else if fname.contains("software")
                || hive.root.name.eq_ignore_ascii_case("SOFTWARE")
                || hive.root.get_path("WOW6432Node").is_some()
                || hive.root.get_path("Wow6432Node").is_some()
            {
                software_hives.push(hive);
            } else if has_office {
                ntuser_hives.push(hive);
            } else {
                software_hives.push(hive);
            }
        }
        Ok(Self {
            ntuser_hives,
            software_hives,
        })
    }

    fn resolve_nodes(&self, root: HiveRoot, path: &str) -> OutlookResult<Vec<&RegKeyNode>> {
        match root {
            HiveRoot::Hkcu => {
                if self.ntuser_hives.is_empty() {
                    return Err(OutlookError::NotFound("No NTUSER.DAT offline hive loaded".to_string()));
                }
                let nodes: Vec<&RegKeyNode> = self
                    .ntuser_hives
                    .iter()
                    .filter_map(|hive| hive.root.get_path(path))
                    .collect();
                if nodes.is_empty() {
                    Err(OutlookError::NotFound(format!("HKCU\\{path}")))
                } else {
                    Ok(nodes)
                }
            }
            HiveRoot::Hklm | HiveRoot::HklmWow64 => {
                if self.software_hives.is_empty() {
                    return Err(OutlookError::NotFound("No SOFTWARE offline hive loaded".to_string()));
                }
                let nodes: Vec<&RegKeyNode> = match root {
                    HiveRoot::Hklm => {
                        let effective = strip_software_prefix(path);
                        self.software_hives
                            .iter()
                            .filter_map(|hive| hive.root.get_path(&effective))
                            .collect()
                    }
                    HiveRoot::HklmWow64 => {
                        let rest = strip_software_prefix(path);
                        self.software_hives
                            .iter()
                            .filter_map(|hive| {
                                hive.root
                                    .get_path(&format!("WOW6432Node\\{rest}"))
                                    .or_else(|| hive.root.get_path(&format!("Wow6432Node\\{rest}")))
                            })
                            .collect()
                    }
                    HiveRoot::Hkcu => unreachable!(),
                };
                if nodes.is_empty() {
                    let prefix = if matches!(root, HiveRoot::HklmWow64) {
                        "HKLM_WOW64"
                    } else {
                        "HKLM"
                    };
                    Err(OutlookError::NotFound(format!("{prefix}\\{path}")))
                } else {
                    Ok(nodes)
                }
            }
        }
    }
}

fn strip_software_prefix(path: &str) -> String {
    let trimmed = path.trim_start_matches(['\\', '/']);
    if let Some(rest) = trimmed
        .strip_prefix("SOFTWARE\\")
        .or_else(|| trimmed.strip_prefix("Software\\"))
    {
        rest.to_string()
    } else if trimmed.eq_ignore_ascii_case("SOFTWARE") {
        String::new()
    } else {
        trimmed.to_string()
    }
}

impl RegistryBackend for OfflineRegfBackend {
    fn enum_subkeys(&self, root: HiveRoot, path: &str) -> OutlookResult<Vec<String>> {
        let nodes = self.resolve_nodes(root, path)?;
        let mut names = Vec::new();
        for node in nodes {
            for k in node.subkeys.keys() {
                if !names.iter().any(|existing: &String| existing.eq_ignore_ascii_case(k)) {
                    names.push(k.clone());
                }
            }
        }
        Ok(names)
    }

    fn read_values(&self, root: HiveRoot, path: &str) -> OutlookResult<BTreeMap<String, RegValue>> {
        let nodes = self.resolve_nodes(root, path)?;
        let mut merged = BTreeMap::new();
        for node in nodes {
            for (k, v) in &node.values {
                merged.insert(k.clone(), v.clone());
            }
        }
        Ok(merged)
    }

    fn set_dword(&self, root: HiveRoot, path: &str, _value_name: &str, _val: u32) -> OutlookResult<()> {
        Err(OutlookError::UnsupportedFeature(format!(
            "Offline regf hives are read-only ({}\\{})",
            root.label(),
            path
        )))
    }
}

/// Deterministic in-memory mock backend for unit tests (including `AccessDenied` simulation).
#[derive(Default)]
pub struct MockRegistryBackend {
    pub keys: std::sync::RwLock<HashMap<(HiveRoot, String), BTreeMap<String, RegValue>>>,
    pub denied_prefixes: std::sync::RwLock<Vec<(HiveRoot, String)>>,
}

impl MockRegistryBackend {
    pub fn insert_key(&self, root: HiveRoot, path: &str, values: BTreeMap<String, RegValue>) {
        self.keys
            .write()
            .unwrap()
            .insert((root, path.to_ascii_lowercase()), values);
    }

    pub fn deny_prefix(&self, root: HiveRoot, prefix: &str) {
        self.denied_prefixes
            .write()
            .unwrap()
            .push((root, prefix.to_ascii_lowercase()));
    }

    fn check_denied(&self, root: HiveRoot, path: &str) -> OutlookResult<()> {
        let p_low = path.to_ascii_lowercase();
        for (r, pfx) in self.denied_prefixes.read().unwrap().iter() {
            if *r == root && p_low.starts_with(pfx) {
                return Err(OutlookError::AccessDenied {
                    scope: format!("{}\\{}", root.label(), path),
                    detail: "Simulated AccessDenied (non-elevated)".to_string(),
                });
            }
        }
        Ok(())
    }
}

impl RegistryBackend for MockRegistryBackend {
    fn enum_subkeys(&self, root: HiveRoot, path: &str) -> OutlookResult<Vec<String>> {
        self.check_denied(root, path)?;
        let prefix = format!("{}\\", path.trim_end_matches('\\').to_ascii_lowercase());
        let mut children = Vec::new();
        let keys = self.keys.read().unwrap();
        for (r, kpath) in keys.keys() {
            if *r == root
                && let Some(rest) = kpath.strip_prefix(&prefix)
                && let Some(seg) = rest.split('\\').next()
                && !seg.is_empty()
                && !children.iter().any(|c: &String| c.eq_ignore_ascii_case(seg))
            {
                children.push(seg.to_string());
            }
        }
        if children.is_empty() && !keys.contains_key(&(root, path.to_ascii_lowercase())) {
            return Err(OutlookError::NotFound(format!("{}\\{}", root.label(), path)));
        }
        Ok(children)
    }

    fn read_values(&self, root: HiveRoot, path: &str) -> OutlookResult<BTreeMap<String, RegValue>> {
        self.check_denied(root, path)?;
        self.keys
            .read()
            .unwrap()
            .get(&(root, path.to_ascii_lowercase()))
            .cloned()
            .ok_or_else(|| OutlookError::NotFound(format!("{}\\{}", root.label(), path)))
    }

    fn set_dword(&self, root: HiveRoot, path: &str, value_name: &str, val: u32) -> OutlookResult<()> {
        self.check_denied(root, path)?;
        let mut guard = self.keys.write().unwrap();
        let entry = guard
            .get_mut(&(root, path.to_ascii_lowercase()))
            .ok_or_else(|| OutlookError::NotFound(format!("{}\\{}", root.label(), path)))?;
        entry.insert(value_name.to_string(), RegValue::Dword(val));
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfficeAddinRecord {
    pub row_id: u64,
    pub office_app: String,
    pub addin_name: String,
    pub prog_id: String,
    pub friendly_name: String,
    pub description: String,
    pub load_behavior: i32,
    pub dll_path: String,
    pub signed: bool,
    pub enabled: bool,
    pub hive_location: String,
    pub registry_path: String,
    pub addin_kind: String,
    pub clsid: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OfficeScanReport {
    pub addins: Vec<OfficeAddinRecord>,
    pub warnings: Vec<ScanWarning>,
}

const OFFICE_APPS: &[&str] = &["Outlook", "Word", "Excel", "PowerPoint", "Access"];
const OFFICE_VERSIONS: &[&str] = &["14.0", "15.0", "16.0"];

/// Scans Office/Outlook add-ins across Registry hives and `%APPDATA%\Microsoft\*` startup folders.
pub fn scan_office_addins(backend: &dyn RegistryBackend, appdata_microsoft_dir: Option<&Path>) -> OfficeScanReport {
    let mut addins = Vec::new();
    let mut warnings = Vec::new();

    for root in [HiveRoot::Hkcu, HiveRoot::Hklm, HiveRoot::HklmWow64] {
        for app in OFFICE_APPS {
            let path = format!("Software\\Microsoft\\Office\\{app}\\Addins");
            scan_addins_key(backend, root, app, &path, &mut addins, &mut warnings);

            for ver in OFFICE_VERSIONS {
                let ver_path = format!("Software\\Microsoft\\Office\\{ver}\\{app}\\Addins");
                scan_addins_key(backend, root, app, &ver_path, &mut addins, &mut warnings);
            }
        }

        for ver in OFFICE_VERSIONS {
            // Excel\Options OPEN* XLL/XLAM entries
            let excel_opts = format!("Software\\Microsoft\\Office\\{ver}\\Excel\\Options");
            match backend.read_values(root, &excel_opts) {
                Ok(vals) => {
                    for (vname, vval) in vals {
                        if vname.to_ascii_uppercase().starts_with("OPEN")
                            && let Some(raw_s) = vval.as_str()
                        {
                            let dll_path = raw_s.trim().trim_start_matches("/R ").trim_matches('"').to_string();
                            let signed = check_pe_authenticode_present(Path::new(&dll_path));
                            addins.push(OfficeAddinRecord {
                                row_id: addins.len() as u64,
                                office_app: "Excel".to_string(),
                                addin_name: vname.clone(),
                                prog_id: vname.clone(),
                                friendly_name: Path::new(&dll_path)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or(&vname)
                                    .to_string(),
                                description: format!("Excel Options {vname} add-in"),
                                load_behavior: 3,
                                dll_path,
                                signed,
                                enabled: true,
                                hive_location: root.label().to_string(),
                                registry_path: format!("{}\\{}", root.label(), excel_opts),
                                addin_kind: "XLL/XLAM (Excel Options)".to_string(),
                                clsid: None,
                            });
                        }
                    }
                }
                Err(OutlookError::AccessDenied { scope, detail }) => {
                    warnings.push(ScanWarning::new(scope, detail));
                }
                Err(_) => {}
            }

            // Outlook\Resiliency\DisabledItems
            let dis_path = format!("Software\\Microsoft\\Office\\{ver}\\Outlook\\Resiliency\\DisabledItems");
            match backend.read_values(root, &dis_path) {
                Ok(vals) => {
                    for (vname, vval) in vals {
                        if let RegValue::Binary(bytes) = vval {
                            let label = extract_disabled_item_label(&bytes).unwrap_or_else(|| vname.clone());
                            addins.push(OfficeAddinRecord {
                                row_id: addins.len() as u64,
                                office_app: "Outlook".to_string(),
                                addin_name: label.clone(),
                                prog_id: label.clone(),
                                friendly_name: label,
                                description: "Outlook Resiliency Disabled Item".to_string(),
                                load_behavior: 0,
                                dll_path: String::new(),
                                signed: false,
                                enabled: false,
                                hive_location: root.label().to_string(),
                                registry_path: format!("{}\\{}", root.label(), dis_path),
                                addin_kind: "Resiliency DisabledItem".to_string(),
                                clsid: None,
                            });
                        }
                    }
                }
                Err(OutlookError::AccessDenied { scope, detail }) => {
                    warnings.push(ScanWarning::new(scope, detail));
                }
                Err(_) => {}
            }
        }
    }

    // AppData template / startup file scan
    if let Some(ms_dir) = appdata_microsoft_dir {
        scan_appdata_office_files(ms_dir, &mut addins);
    }

    for (idx, rec) in addins.iter_mut().enumerate() {
        rec.row_id = idx as u64;
    }

    OfficeScanReport { addins, warnings }
}

fn scan_addins_key(
    backend: &dyn RegistryBackend,
    root: HiveRoot,
    app: &str,
    base_path: &str,
    out: &mut Vec<OfficeAddinRecord>,
    warnings: &mut Vec<ScanWarning>,
) {
    let subkeys = match backend.enum_subkeys(root, base_path) {
        Ok(k) => k,
        Err(OutlookError::AccessDenied { scope, detail }) => {
            warnings.push(ScanWarning::new(scope, detail));
            return;
        }
        Err(_) => return,
    };

    for prog_id in subkeys {
        let full_path = format!("{base_path}\\{prog_id}");
        let vals = match backend.read_values(root, &full_path) {
            Ok(v) => v,
            Err(OutlookError::AccessDenied { scope, detail }) => {
                warnings.push(ScanWarning::new(scope, detail));
                continue;
            }
            Err(_) => continue,
        };

        let friendly = vals
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("FriendlyName"))
            .and_then(|(_, v)| v.as_str())
            .unwrap_or(&prog_id)
            .to_string();
        let description = vals
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Description"))
            .and_then(|(_, v)| v.as_str())
            .unwrap_or("")
            .to_string();
        let load_behavior = vals
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("LoadBehavior"))
            .and_then(|(_, v)| v.as_u32())
            .map(|v| v as i32)
            .unwrap_or(0);
        let manifest = vals
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("Manifest"))
            .and_then(|(_, v)| v.as_str())
            .map(|s| s.to_string());

        let (clsid, resolved_dll) = resolve_com_clsid_dll(backend, &prog_id);
        let dll_path = manifest.clone().filter(|s| !s.is_empty()).unwrap_or(resolved_dll);
        let addin_kind = if manifest.is_some() { "VSTO" } else { "COM" }.to_string();
        let clean_dll_for_pe = dll_path.strip_prefix("file:///").unwrap_or(&dll_path);
        let signed = check_pe_authenticode_present(Path::new(clean_dll_for_pe));
        let enabled = (load_behavior & 0x03) != 0 || load_behavior == 3;

        out.push(OfficeAddinRecord {
            row_id: out.len() as u64,
            office_app: app.to_string(),
            addin_name: friendly.clone(),
            prog_id,
            friendly_name: friendly,
            description,
            load_behavior,
            dll_path,
            signed,
            enabled,
            hive_location: root.label().to_string(),
            registry_path: format!("{}\\{}", root.label(), full_path),
            addin_kind,
            clsid,
        });
    }
}

fn resolve_com_clsid_dll(backend: &dyn RegistryBackend, prog_id: &str) -> (Option<String>, String) {
    for root in [HiveRoot::Hkcu, HiveRoot::Hklm, HiveRoot::HklmWow64] {
        let clsid_key = format!("Software\\Classes\\{prog_id}\\CLSID");
        if let Ok(vals) = backend.read_values(root, &clsid_key)
            && let Some(clsid) = vals.get("").and_then(|v| v.as_str())
        {
            for srv_root in [root, HiveRoot::Hklm, HiveRoot::HklmWow64, HiveRoot::Hkcu] {
                for sub in ["InprocServer32", "LocalServer32"] {
                    let srv_path = format!("Software\\Classes\\CLSID\\{clsid}\\{sub}");
                    if let Ok(svals) = backend.read_values(srv_root, &srv_path)
                        && let Some(dll) = svals.get("").and_then(|v| v.as_str())
                    {
                        return (Some(clsid.to_string()), dll.to_string());
                    }
                }
            }
            return (Some(clsid.to_string()), String::new());
        }
    }
    (None, String::new())
}

fn extract_disabled_item_label(bytes: &[u8]) -> Option<String> {
    // DisabledItems binary format: 12-byte header (type, path_bytes, name_bytes), followed by UTF-16LE strings
    if bytes.len() < 16 {
        return None;
    }
    let s = crate::time::decode_utf16le(&bytes[12..]);
    if s.is_empty() { None } else { Some(s) }
}

fn scan_appdata_office_files(ms_dir: &Path, out: &mut Vec<OfficeAddinRecord>) {
    let candidates = [
        ("Word", ms_dir.join("Word").join("STARTUP")),
        ("Excel", ms_dir.join("Excel").join("XLSTART")),
        ("Office", ms_dir.join("AddIns")),
        ("Office", ms_dir.join("Templates")),
        ("Outlook", ms_dir.join("Outlook")),
    ];

    for (app, dir) in candidates {
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&dir).max_depth(2).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let ext = p
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let is_match = matches!(ext.as_str(), "dotm" | "dot" | "wll" | "xll" | "xlam" | "ppam")
                || fname.eq_ignore_ascii_case("VbaProject.OTM");
            if !is_match {
                continue;
            }
            let signed = if matches!(ext.as_str(), "wll" | "xll" | "dll") {
                check_pe_authenticode_present(p)
            } else {
                false
            };
            out.push(OfficeAddinRecord {
                row_id: out.len() as u64,
                office_app: app.to_string(),
                addin_name: fname.to_string(),
                prog_id: fname.to_string(),
                friendly_name: fname.to_string(),
                description: format!("Startup/Template file ({})", ext.to_ascii_uppercase()),
                load_behavior: 3,
                dll_path: p.display().to_string(),
                signed,
                enabled: true,
                hive_location: "AppData".to_string(),
                registry_path: p.display().to_string(),
                addin_kind: format!("File ({})", ext.to_ascii_uppercase()),
                clsid: None,
            });
        }
    }
}

/// Pure-Rust PE `IMAGE_DIRECTORY_ENTRY_SECURITY` (`WIN_CERTIFICATE`) signature detector.
/// Returns `true` if `path` is a valid PE32/PE32+ binary with a non-empty PKCS#7 Authenticode directory.
pub fn check_pe_authenticode_present(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    check_pe_authenticode_bytes(&bytes)
}

pub fn check_pe_authenticode_bytes(bytes: &[u8]) -> bool {
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return false;
    }
    let e_lfanew = u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()) as usize;
    if e_lfanew + 24 + 96 > bytes.len() || &bytes[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return false;
    }
    let opt_off = e_lfanew + 24;
    let magic = u16::from_le_bytes([bytes[opt_off], bytes[opt_off + 1]]);
    let (num_rva_off, data_dir_off) = match magic {
        0x010B => (opt_off + 92, opt_off + 96),   // PE32
        0x020B => (opt_off + 108, opt_off + 112), // PE32+
        _ => return false,
    };
    if num_rva_off + 4 > bytes.len() {
        return false;
    }
    let num_rva_and_sizes = u32::from_le_bytes(bytes[num_rva_off..num_rva_off + 4].try_into().unwrap());
    if num_rva_and_sizes <= 4 {
        return false;
    }
    // IMAGE_DIRECTORY_ENTRY_SECURITY is index 4 (each entry is 8 bytes: VirtualAddress/FileOffset, Size)
    let sec_entry_off = data_dir_off + 4 * 8;
    if sec_entry_off + 8 > bytes.len() {
        return false;
    }
    let file_off = u32::from_le_bytes(bytes[sec_entry_off..sec_entry_off + 4].try_into().unwrap()) as usize;
    let cert_size = u32::from_le_bytes(bytes[sec_entry_off + 4..sec_entry_off + 8].try_into().unwrap()) as usize;
    if file_off == 0 || cert_size <= 8 || file_off + cert_size > bytes.len() {
        return false;
    }
    let dw_length = u32::from_le_bytes(bytes[file_off..file_off + 4].try_into().unwrap()) as usize;
    let w_cert_type = u16::from_le_bytes([bytes[file_off + 6], bytes[file_off + 7]]);
    dw_length > 8 && dw_length <= cert_size && w_cert_type == 0x0002
}

/// Toggles an Office add-in's `LoadBehavior` DWORD (`3` = enabled, `0` = disabled) via `RegistryBackend`.
pub fn toggle_office_addin(
    backend: &dyn RegistryBackend,
    prog_id: &str,
    app_filter: Option<&str>,
    enable: bool,
    dry_run: bool,
) -> OutlookResult<(String, i32, i32)> {
    let report = scan_office_addins(backend, None);
    let target = report
        .addins
        .into_iter()
        .find(|a| {
            a.prog_id.eq_ignore_ascii_case(prog_id)
                && app_filter.is_none_or(|app| a.office_app.eq_ignore_ascii_case(app))
        })
        .ok_or_else(|| OutlookError::NotFound(format!("Office add-in '{prog_id}' not found")))?;

    let (root, rel_path) = parse_registry_path(&target.registry_path)?;
    let new_behavior: u32 = if enable { 3 } else { 0 };

    if !dry_run {
        backend.set_dword(root, &rel_path, "LoadBehavior", new_behavior)?;
    }
    Ok((target.registry_path, target.load_behavior, new_behavior as i32))
}

fn parse_registry_path(full: &str) -> OutlookResult<(HiveRoot, String)> {
    if let Some(rest) = full.strip_prefix("HKCU\\") {
        Ok((HiveRoot::Hkcu, rest.to_string()))
    } else if let Some(rest) = full.strip_prefix("HKLM (64-bit)\\") {
        Ok((HiveRoot::Hklm, rest.to_string()))
    } else if let Some(rest) = full.strip_prefix("HKLM (WOW6432Node)\\") {
        Ok((HiveRoot::HklmWow64, rest.to_string()))
    } else {
        Err(OutlookError::UnsupportedFeature(format!(
            "Cannot toggle non-Registry add-in entry: {full}"
        )))
    }
}

pub fn default_appdata_microsoft_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("Microsoft"))
}

/// `guap-table::TableProvider` for Office & Outlook add-ins (`mail2rust_office_addins`).
pub struct OfficeAddinsTableProvider {
    schema: TableSchema,
    rows: Vec<Row>,
    pub warnings: Vec<ScanWarning>,
}

impl OfficeAddinsTableProvider {
    pub fn from_report(report: OfficeScanReport) -> Self {
        let schema = TableSchema::new(
            "mail2rust_office_addins",
            "Microsoft Office & Outlook Add-ins",
            vec![
                ColumnDef::new("office_app", "Office App", ColumnType::Text).with_width_hint(95),
                ColumnDef::new("addin_name", "Add-in Name", ColumnType::Text).with_width_hint(190),
                ColumnDef::new("prog_id", "ProgID", ColumnType::Text).with_width_hint(200),
                ColumnDef::new("friendly_name", "Friendly Name", ColumnType::Text).with_width_hint(180),
                ColumnDef::new("description", "Description", ColumnType::Text).with_width_hint(210),
                ColumnDef::new("load_behavior", "LoadBehavior", ColumnType::Int).with_width_hint(95),
                ColumnDef::new("dll_path", "DLL / Manifest Path", ColumnType::Text).with_width_hint(240),
                ColumnDef::new("signed", "Signed", ColumnType::Bool).with_width_hint(70),
                ColumnDef::new("enabled", "Enabled", ColumnType::Bool).with_width_hint(75),
            ],
        )
        .with_default_sort(0, guap_table::SortDirection::Asc);

        let rows = report
            .addins
            .iter()
            .map(|a| {
                let detail = json!({
                    "hive_location": a.hive_location,
                    "registry_path": a.registry_path,
                    "addin_kind": a.addin_kind,
                    "clsid": a.clsid,
                    "warnings_count": report.warnings.len(),
                });
                Row::new(
                    a.row_id,
                    vec![
                        CellValue::Text(a.office_app.clone()),
                        CellValue::Text(a.addin_name.clone()),
                        CellValue::Text(a.prog_id.clone()),
                        CellValue::Text(a.friendly_name.clone()),
                        CellValue::Text(a.description.clone()),
                        CellValue::Int(i64::from(a.load_behavior)),
                        CellValue::Text(a.dll_path.clone()),
                        CellValue::Bool(a.signed),
                        CellValue::Bool(a.enabled),
                    ],
                )
                .with_detail_json(detail)
            })
            .collect();

        Self {
            schema,
            rows,
            warnings: report.warnings,
        }
    }

    pub fn scan_with_backend(backend: &dyn RegistryBackend, appdata_ms: Option<&Path>) -> Self {
        Self::from_report(scan_office_addins(backend, appdata_ms))
    }

    pub fn scan_from_hives(hive_paths: &[PathBuf], appdata_ms: Option<&Path>) -> OutlookResult<Self> {
        if hive_paths.is_empty() {
            let default_ad = default_appdata_microsoft_dir();
            let effective_ad = appdata_ms.or(default_ad.as_deref());
            #[cfg(windows)]
            {
                let backend = LiveWindowsRegistry;
                return Ok(Self::scan_with_backend(&backend, effective_ad));
            }
            #[cfg(not(windows))]
            {
                let _ = effective_ad;
                return Err(OutlookError::UnsupportedFeature(
                    "Live Windows Registry unavailable on non-Windows; pass offline NTUSER.DAT/SOFTWARE hive paths"
                        .to_string(),
                ));
            }
        }
        let backend = OfflineRegfBackend::from_hive_paths(hive_paths)?;
        Ok(Self::scan_with_backend(&backend, appdata_ms))
    }
}

impl TableProvider for OfficeAddinsTableProvider {
    fn tool_id(&self) -> &'static str {
        "mail2rust_office_addins"
    }

    fn description(&self) -> &'static str {
        "Enumerate Microsoft Office & Outlook add-ins (COM/VSTO/XLL/WLL/DOTM/OTM) from live Registry or offline regf hives"
    }

    fn schema(&self) -> TableSchema {
        self.schema.clone()
    }

    fn query(&self, req: &TableQuery) -> Result<TableData, TableError> {
        apply_query(&self.schema, &self.rows, req)
    }
}

/// Read-only MCP `ActionCommand` for `mail2rust_office_addins`.
pub struct OfficeAddinsListCommand;

impl ActionCommand for OfficeAddinsListCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_office_addins"
    }

    fn description(&self) -> &'static str {
        "List Microsoft Office & Outlook add-ins from live Windows Registry or offline NTUSER.DAT/SOFTWARE regf hives (read-only)."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::ReadOnly
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "hives": { "type": "array", "items": { "type": "string" }, "description": "Optional offline NTUSER.DAT / SOFTWARE regf hive paths" },
                "appdata_microsoft": { "type": "string", "description": "Optional Microsoft AppData directory override" },
                "filter": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 },
                "offset": { "type": "integer", "minimum": 0 }
            }
        })
    }

    fn execute(&self, args: &Value, _dry_run: bool) -> Result<Value, McpToolError> {
        let hives: Vec<PathBuf> = args
            .get("hives")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(PathBuf::from).collect())
            .unwrap_or_default();
        let appdata_override = args.get("appdata_microsoft").and_then(Value::as_str).map(PathBuf::from);

        let provider = if hives.is_empty() {
            let effective_appdata = appdata_override.or_else(default_appdata_microsoft_dir);
            #[cfg(windows)]
            {
                OfficeAddinsTableProvider::scan_with_backend(&LiveWindowsRegistry, effective_appdata.as_deref())
            }
            #[cfg(not(windows))]
            {
                let _ = effective_appdata;
                return Err(McpToolError::ExecutionFailed(
                    "Live Windows Registry requires Windows; pass 'hives' for offline inspection".to_string(),
                ));
            }
        } else {
            let backend = OfflineRegfBackend::from_hive_paths(&hives)
                .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
            OfficeAddinsTableProvider::scan_with_backend(&backend, appdata_override.as_deref())
        };

        let mut res = query_provider_to_json(&provider, args)?;
        if let Some(obj) = res.as_object_mut() {
            obj.insert("warnings".to_string(), json!(provider.warnings));
        }
        Ok(res)
    }
}

/// Mutating MCP `ActionCommand` for `mail2rust_office_addins_toggle`.
pub struct OfficeAddinsToggleCommand;

impl ActionCommand for OfficeAddinsToggleCommand {
    fn command_id(&self) -> &'static str {
        "mail2rust_office_addins_toggle"
    }

    fn description(&self) -> &'static str {
        "Enable or disable an Office/Outlook add-in LoadBehavior in live Windows Registry (mutating; requires AllowMutating + confirm_destructive=true)."
    }

    fn risk_level(&self) -> ActionRiskLevel {
        ActionRiskLevel::Mutating
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prog_id": { "type": "string", "description": "Add-in ProgID (e.g. Titan.OutlookAddin)" },
                "app": { "type": "string", "description": "Optional Office app filter (Outlook, Word, Excel, PowerPoint, Access)" },
                "enable": { "type": "boolean", "description": "true sets LoadBehavior=3; false sets LoadBehavior=0" }
            },
            "required": ["prog_id", "enable"]
        })
    }

    fn execute(&self, args: &Value, dry_run: bool) -> Result<Value, McpToolError> {
        let prog_id = args
            .get("prog_id")
            .and_then(Value::as_str)
            .ok_or_else(|| McpToolError::InvalidParams("Missing required 'prog_id'".to_string()))?;
        let enable = args
            .get("enable")
            .and_then(Value::as_bool)
            .ok_or_else(|| McpToolError::InvalidParams("Missing required boolean 'enable'".to_string()))?;
        let app = args.get("app").and_then(Value::as_str);

        #[cfg(windows)]
        {
            let backend = LiveWindowsRegistry;
            let (reg_path, old_val, new_val) = toggle_office_addin(&backend, prog_id, app, enable, dry_run)
                .map_err(|e| McpToolError::ExecutionFailed(e.to_string()))?;
            Ok(json!({
                "status": if dry_run { "dry_run" } else { "updated" },
                "prog_id": prog_id,
                "registry_path": reg_path,
                "previous_load_behavior": old_val,
                "new_load_behavior": new_val,
            }))
        }
        #[cfg(not(windows))]
        {
            let _ = (prog_id, enable, app, dry_run);
            Err(McpToolError::ExecutionFailed(
                "Live Registry toggle is only available on Windows".to_string(),
            ))
        }
    }
}
