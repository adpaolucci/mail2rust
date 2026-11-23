use std::path::PathBuf;
use winsafe::{self as w, co, prelude::*};

fn apply_initial_dir(dlg: &impl shell_IFileDialog, initial_dir: Option<&str>) {
    if let Some(dir_str) = initial_dir {
        let trimmed = dir_str.trim().trim_matches('"').trim();
        if trimmed.is_empty() {
            return;
        }
        let p = PathBuf::from(trimmed);
        let candidate = if p.is_dir() {
            Some(p)
        } else {
            p.parent().filter(|par| par.is_dir()).map(|par| par.to_path_buf())
        };
        if let Some(dir) = candidate
            && let Ok(folder_item) =
                w::SHCreateItemFromParsingName::<w::IShellItem>(&dir.display().to_string(), None::<&w::IBindCtx>)
        {
            let _ = dlg.SetFolder(&folder_item);
        }
    }
}

pub fn pick_open_file(
    parent: &w::HWND,
    title: &str,
    filters: &[(&str, &str)],
    initial_dir: Option<&str>,
) -> w::AnyResult<Option<PathBuf>> {
    let dlg = w::CoCreateInstance::<w::IFileOpenDialog>(
        &co::CLSID::FileOpenDialog,
        None::<&w::IUnknown>,
        co::CLSCTX::INPROC_SERVER,
    )?;
    dlg.SetTitle(title)?;
    dlg.SetOptions(dlg.GetOptions()? | co::FOS::FORCEFILESYSTEM | co::FOS::FILEMUSTEXIST)?;
    if !filters.is_empty() {
        dlg.SetFileTypes(filters)?;
        dlg.SetFileTypeIndex(1)?;
    }
    apply_initial_dir(&dlg, initial_dir);
    if dlg.Show(parent)? {
        let item = dlg.GetResult()?;
        let path = item.GetDisplayName(co::SIGDN::FILESYSPATH)?;
        return Ok(Some(PathBuf::from(path)));
    }
    Ok(None)
}

pub fn pick_folder(parent: &w::HWND, title: &str, initial_dir: Option<&str>) -> w::AnyResult<Option<PathBuf>> {
    let dlg = w::CoCreateInstance::<w::IFileOpenDialog>(
        &co::CLSID::FileOpenDialog,
        None::<&w::IUnknown>,
        co::CLSCTX::INPROC_SERVER,
    )?;
    dlg.SetTitle(title)?;
    dlg.SetOptions(dlg.GetOptions()? | co::FOS::FORCEFILESYSTEM | co::FOS::PICKFOLDERS)?;
    apply_initial_dir(&dlg, initial_dir);
    if dlg.Show(parent)? {
        let item = dlg.GetResult()?;
        let path = item.GetDisplayName(co::SIGDN::FILESYSPATH)?;
        return Ok(Some(PathBuf::from(path)));
    }
    Ok(None)
}

pub fn pick_save_file(
    parent: &w::HWND,
    title: &str,
    default_name: &str,
    default_ext: &str,
    filters: &[(&str, &str)],
) -> w::AnyResult<Option<PathBuf>> {
    let dlg = w::CoCreateInstance::<w::IFileSaveDialog>(
        &co::CLSID::FileSaveDialog,
        None::<&w::IUnknown>,
        co::CLSCTX::INPROC_SERVER,
    )?;
    dlg.SetTitle(title)?;
    dlg.SetOptions(dlg.GetOptions()? | co::FOS::FORCEFILESYSTEM | co::FOS::OVERWRITEPROMPT)?;
    if !filters.is_empty() {
        dlg.SetFileTypes(filters)?;
        dlg.SetFileTypeIndex(1)?;
    }
    if !default_name.is_empty() {
        dlg.SetFileName(default_name)?;
    }
    if !default_ext.is_empty() {
        dlg.SetDefaultExtension(default_ext)?;
    }
    if dlg.Show(parent)? {
        let item = dlg.GetResult()?;
        let path = item.GetDisplayName(co::SIGDN::FILESYSPATH)?;
        return Ok(Some(PathBuf::from(path)));
    }
    Ok(None)
}

pub fn copy_to_clipboard(text: &str) -> bool {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        return cb.set_text(text.to_string()).is_ok();
    }
    false
}
