use crate::LaunchArgs;
use crate::win32_dialogs::{copy_to_clipboard, pick_folder, pick_open_file, pick_open_files, pick_save_file};
use crate::win32_state::{ExportFormat, ExportScope, PreviewTab, SortColumn, Win32State, format_size};
use mail2rust_core::EmailMessage;
use mail2rust_pst::PstEncryption;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use winsafe::{self as w, co, gui, msg, prelude::*};

// Menu command IDs
const IDM_OPEN_FILE: u16 = 4001;
const IDM_OPEN_FOLDER: u16 = 4002;
const IDM_LOAD_SAMPLE: u16 = 4003;
const IDM_EXPORT_ALL: u16 = 4004;
const IDM_EXPORT_FILTERED: u16 = 4005;
const IDM_EXPORT_SELECTED: u16 = 4006;
const IDM_EXIT: u16 = 4007;

const IDM_SELECT_ALL: u16 = 4010;
const IDM_INVERT_SEL: u16 = 4011;
const IDM_CLEAR_SEL: u16 = 4012;
const IDM_TOGGLE_ROW_SEL: u16 = 4013;

const IDM_PREV_BODY: u16 = 4020;
const IDM_PREV_HEADERS: u16 = 4021;
const IDM_PREV_MAPI: u16 = 4022;
const IDM_PREV_FORENSICS: u16 = 4023;
const IDM_REFRESH: u16 = 4024;
const IDM_CLEAR_SEARCH: u16 = 4025;

const IDM_TAB_BASE: u16 = 4030;
const IDM_CLEAR_DATA: u16 = 4049;
const IDM_ABOUT: u16 = 4050;

const MAIL_OPEN_FILTERS: &[(&str, &str)] = &[
    (
        "All Supported Mail Files",
        "*.pst;*.ost;*.mbox;*.mbx;*.eml;*.emlx;*.msg;*.olm;*.zip;*.gz;*.bz2;*.xz;*.tar;*.tgz",
    ),
    ("Outlook PST / OST", "*.pst;*.ost"),
    ("MBOX Mailbox", "*.mbox;*.mbx"),
    ("Email Messages", "*.eml;*.emlx;*.msg"),
    ("All Files", "*.*"),
];

fn to_crlf(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

fn lbl(parent: &(impl GuiParent + 'static), text: &str, x: i32, y: i32, w_dp: i32) -> gui::Label {
    gui::Label::new(
        parent,
        gui::LabelOpts {
            text,
            position: gui::dpi(x, y),
            size: gui::dpi(w_dp, 18),
            ..Default::default()
        },
    )
}

fn edit(
    parent: &(impl GuiParent + 'static),
    x: i32,
    y: i32,
    w_dp: i32,
    h_dp: i32,
    resize: (gui::Horz, gui::Vert),
) -> gui::Edit {
    gui::Edit::new(
        parent,
        gui::EditOpts {
            position: gui::dpi(x, y),
            width: gui::dpi_x(w_dp),
            height: gui::dpi_y(h_dp),
            resize_behavior: resize,
            ..Default::default()
        },
    )
}

fn edit_pass(parent: &(impl GuiParent + 'static), x: i32, y: i32, w_dp: i32, h_dp: i32) -> gui::Edit {
    gui::Edit::new(
        parent,
        gui::EditOpts {
            position: gui::dpi(x, y),
            width: gui::dpi_x(w_dp),
            height: gui::dpi_y(h_dp),
            control_style: co::ES::PASSWORD | co::ES::AUTOHSCROLL,
            ..Default::default()
        },
    )
}

fn chk(parent: &(impl GuiParent + 'static), text: &str, x: i32, y: i32) -> gui::CheckBox {
    gui::CheckBox::new(
        parent,
        gui::CheckBoxOpts {
            text,
            position: gui::dpi(x, y),
            ..Default::default()
        },
    )
}

fn btn(
    parent: &(impl GuiParent + 'static),
    text: &str,
    x: i32,
    y: i32,
    w_dp: i32,
    h_dp: i32,
    resize: (gui::Horz, gui::Vert),
) -> gui::Button {
    gui::Button::new(
        parent,
        gui::ButtonOpts {
            text,
            position: gui::dpi(x, y),
            width: gui::dpi_x(w_dp),
            height: gui::dpi_y(h_dp),
            resize_behavior: resize,
            ..Default::default()
        },
    )
}

fn combo(parent: &(impl GuiParent + 'static), x: i32, y: i32, w_dp: i32, items: &[&str]) -> gui::ComboBox {
    gui::ComboBox::new(
        parent,
        gui::ComboBoxOpts {
            position: gui::dpi(x, y),
            width: gui::dpi_x(w_dp),
            items,
            selected_item: (!items.is_empty()).then_some(0),
            ..Default::default()
        },
    )
}

fn tab_page(parent: &(impl GuiParent + 'static)) -> gui::TabPage {
    gui::TabPage::new(
        parent,
        gui::TabPageOpts {
            class_bg_brush: gui::Brush::Color(co::COLOR::BTNFACE),
            ..Default::default()
        },
    )
}

fn move_ctrl(ctrl: &impl GuiWindow, x: i32, y: i32, w: i32, h: i32) {
    if *ctrl.hwnd() != w::HWND::NULL {
        let _ = ctrl.hwnd().SetWindowPos(
            w::HwndPlace::None,
            w::POINT::with(x, y),
            w::SIZE::with(w.max(10), h.max(10)),
            co::SWP::NOZORDER,
        );
    }
}

fn make_list(
    parent: &(impl GuiParent + 'static),
    x: i32,
    y: i32,
    w_dp: i32,
    h_dp: i32,
    cols: &[(&str, i32)],
    sort_hdr: bool,
) -> gui::ListView {
    let style = if sort_hdr {
        co::LVS::REPORT | co::LVS::SHOWSELALWAYS
    } else {
        co::LVS::REPORT | co::LVS::NOSORTHEADER | co::LVS::SHOWSELALWAYS
    };
    let scaled_cols: Vec<(&str, i32)> = cols.iter().map(|(t, w)| (*t, gui::dpi_x(*w))).collect();
    gui::ListView::new(
        parent,
        gui::ListViewOpts {
            position: gui::dpi(x, y),
            size: gui::dpi(w_dp, h_dp),
            control_style: style,
            columns: &scaled_cols,
            ..Default::default()
        },
    )
}

fn readonly_multiline(parent: &(impl GuiParent + 'static), x: i32, y: i32, w_dp: i32, h_dp: i32) -> gui::Edit {
    gui::Edit::new(
        parent,
        gui::EditOpts {
            position: gui::dpi(x, y),
            width: gui::dpi_x(w_dp),
            height: gui::dpi_y(h_dp),
            control_style: co::ES::MULTILINE | co::ES::READONLY | co::ES::AUTOVSCROLL | co::ES::AUTOHSCROLL,
            window_style: co::WS::CHILD | co::WS::TABSTOP | co::WS::VISIBLE | co::WS::VSCROLL | co::WS::HSCROLL,
            ..Default::default()
        },
    )
}

#[derive(Clone)]
struct WorkbenchPage {
    page: gui::TabPage,
    folder_combo: gui::ComboBox,
    search_edit: gui::Edit,
    btn_clear_search: gui::Button,
    btn_prev_body: gui::Button,
    btn_prev_hdr: gui::Button,
    btn_prev_mapi: gui::Button,
    btn_prev_for: gui::Button,
    btn_copy_preview: gui::Button,
    btn_toggle_sel: gui::Button,
    list: gui::ListView,
    preview_edit: gui::Edit,
}

impl WorkbenchPage {
    fn new(parent: &(impl GuiParent + 'static)) -> Self {
        let page = tab_page(parent);

        lbl(&page, "Folder:", 10, 12, 46);
        let folder_combo = combo(&page, 58, 9, 175, &["All Mail"]);
        lbl(&page, "Find:", 242, 12, 34);
        let search_edit = edit(&page, 278, 9, 180, 22, (gui::Horz::None, gui::Vert::None));
        let btn_clear_search = btn(&page, "Clear", 464, 8, 52, 24, (gui::Horz::None, gui::Vert::None));

        let btn_prev_body = btn(&page, "Body", 530, 8, 52, 24, (gui::Horz::None, gui::Vert::None));
        let btn_prev_hdr = btn(&page, "Headers", 585, 8, 64, 24, (gui::Horz::None, gui::Vert::None));
        let btn_prev_mapi = btn(&page, "MAPI", 652, 8, 52, 24, (gui::Horz::None, gui::Vert::None));
        let btn_prev_for = btn(&page, "Forensics", 707, 8, 72, 24, (gui::Horz::None, gui::Vert::None));
        let btn_toggle_sel = btn(&page, "Mark Row", 785, 8, 74, 24, (gui::Horz::None, gui::Vert::None));
        let btn_copy_preview = btn(&page, "Copy", 864, 8, 56, 24, (gui::Horz::None, gui::Vert::None));

        let list = make_list(
            &page,
            10,
            38,
            610,
            490,
            &[
                ("Bates / ID", 118),
                ("!", 28),
                ("Flags", 68),
                ("From", 150),
                ("Subject", 230),
                ("Received (UTC)", 130),
                ("Size", 70),
            ],
            true,
        );
        let preview_edit = readonly_multiline(&page, 628, 38, 430, 490);

        {
            let list = list.clone();
            let preview_edit = preview_edit.clone();
            page.on().wm_size(move |p| {
                if p.request == co::SIZE_R::MINIMIZED || p.client_area.cx <= 0 || p.client_area.cy <= 0 {
                    return Ok(());
                }
                let w = p.client_area.cx;
                let h = p.client_area.cy;
                let pad = gui::dpi_x(10);
                let gap = gui::dpi_x(8);
                let top = gui::dpi_y(38);
                let body_h = (h - top - gui::dpi_y(8)).max(gui::dpi_y(120));
                let insp_w = ((w - 2 * pad) * 38 / 100).clamp(gui::dpi_x(280), gui::dpi_x(520));
                let list_w = (w - 2 * pad - gap - insp_w).max(gui::dpi_x(280));
                move_ctrl(&list, pad, top, list_w, body_h);
                move_ctrl(&preview_edit, pad + list_w + gap, top, insp_w, body_h);
                Ok(())
            });
        }

        Self {
            page,
            folder_combo,
            search_edit,
            btn_clear_search,
            btn_prev_body,
            btn_prev_hdr,
            btn_prev_mapi,
            btn_prev_for,
            btn_copy_preview,
            btn_toggle_sel,
            list,
            preview_edit,
        }
    }
}

#[derive(Clone)]
struct ExportPage {
    page: gui::TabPage,
    scope_combo: gui::ComboBox,
    format_combo: gui::ComboBox,
    dest_edit: gui::Edit,
    btn_browse_dest: gui::Button,
    root_edit: gui::Edit,
    max_gb_edit: gui::Edit,
    chk_cluster_threads: gui::CheckBox,
    enc_combo: gui::ComboBox,
    pwd_edit: gui::Edit,
    btn_export: gui::Button,
    btn_auto_split: gui::Button,
    status_edit: gui::Edit,
}

impl ExportPage {
    fn new(parent: &(impl GuiParent + 'static)) -> Self {
        let page = tab_page(parent);

        lbl(&page, "Export Scope:", 12, 16, 110);
        let scope_combo = combo(
            &page,
            130,
            12,
            230,
            &["All Messages", "Current Filtered View", "Marked / Selected Items"],
        );

        lbl(&page, "Target Format:", 385, 16, 100);
        let format_combo = combo(
            &page,
            490,
            12,
            230,
            &[
                "Microsoft Outlook (.pst)",
                "Standard MBOX (.mbox)",
                "RFC 5322 EML Tree Directory",
            ],
        );

        lbl(&page, "Destination:", 12, 48, 110);
        let dest_edit = edit(&page, 130, 44, 480, 22, (gui::Horz::None, gui::Vert::None));
        let btn_browse_dest = btn(&page, "Browse...", 618, 43, 86, 24, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "PST Root Folder:", 12, 80, 110);
        let root_edit = edit(&page, 130, 76, 220, 22, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "Volume Limit (GB):", 385, 80, 115);
        let max_gb_edit = edit(&page, 506, 76, 70, 22, (gui::Horz::None, gui::Vert::None));

        let chk_cluster_threads = chk(
            &page,
            "Preserve email conversation threads across split volumes",
            130,
            110,
        );

        lbl(&page, "PST Encryption:", 12, 142, 110);
        let enc_combo = combo(
            &page,
            130,
            138,
            220,
            &["Compressible (Permute)", "High Encryption (Cyclic)", "None"],
        );

        lbl(&page, "PST Password:", 385, 142, 100);
        let pwd_edit = edit_pass(&page, 490, 138, 210, 22);

        let btn_export = btn(
            &page,
            "Export Container",
            130,
            174,
            150,
            26,
            (gui::Horz::None, gui::Vert::None),
        );
        let btn_auto_split = btn(
            &page,
            "Compute Auto-Split Volumes",
            290,
            174,
            190,
            26,
            (gui::Horz::None, gui::Vert::None),
        );

        lbl(&page, "Export && Split Log:", 12, 212, 160);
        let status_edit = readonly_multiline(&page, 12, 234, 920, 260);

        {
            let status_edit = status_edit.clone();
            page.on().wm_size(move |p| {
                if p.request == co::SIZE_R::MINIMIZED || p.client_area.cx <= 0 || p.client_area.cy <= 0 {
                    return Ok(());
                }
                let pad = gui::dpi_x(12);
                let w = (p.client_area.cx - 2 * pad).max(gui::dpi_x(260));
                let y = gui::dpi_y(234);
                let h = (p.client_area.cy - y - gui::dpi_y(10)).max(gui::dpi_y(100));
                move_ctrl(&status_edit, pad, y, w, h);
                Ok(())
            });
        }

        Self {
            page,
            scope_combo,
            format_combo,
            dest_edit,
            btn_browse_dest,
            root_edit,
            max_gb_edit,
            chk_cluster_threads,
            enc_combo,
            pwd_edit,
            btn_export,
            btn_auto_split,
            status_edit,
        }
    }
}

#[derive(Clone)]
struct ImapPage {
    page: gui::TabPage,
    preset_gmail: gui::Button,
    preset_outlook: gui::Button,
    preset_yahoo: gui::Button,
    preset_proton: gui::Button,
    preset_icloud: gui::Button,
    preset_fastmail: gui::Button,
    host_edit: gui::Edit,
    port_edit: gui::Edit,
    auth_combo: gui::ComboBox,
    user_edit: gui::Edit,
    pass_edit: gui::Edit,
    oauth_edit: gui::Edit,
    chk_tls: gui::CheckBox,
    chk_insecure: gui::CheckBox,
    folders_edit: gui::Edit,
    max_edit: gui::Edit,
    btn_connect: gui::Button,

    sync_src_host: gui::Edit,
    sync_src_port: gui::Edit,
    sync_src_user: gui::Edit,
    sync_src_auth_combo: gui::ComboBox,
    sync_src_pass: gui::Edit,
    sync_src_oauth: gui::Edit,
    sync_src_chk_tls: gui::CheckBox,
    sync_dst_host: gui::Edit,
    sync_dst_port: gui::Edit,
    sync_dst_user: gui::Edit,
    sync_dst_auth_combo: gui::ComboBox,
    sync_dst_pass: gui::Edit,
    sync_dst_oauth: gui::Edit,
    sync_dst_chk_tls: gui::CheckBox,
    sync_chk_insecure: gui::CheckBox,
    sync_folders: gui::Edit,
    sync_prefix: gui::Edit,
    sync_chk_flags: gui::CheckBox,
    sync_chk_dates: gui::CheckBox,
    sync_chk_dry_run: gui::CheckBox,
    btn_sync: gui::Button,
    status_edit: gui::Edit,
}

impl ImapPage {
    fn new(parent: &(impl GuiParent + 'static)) -> Self {
        let page = tab_page(parent);

        lbl(&page, "Presets:", 12, 14, 58);
        let preset_gmail = btn(&page, "Gmail", 74, 10, 60, 22, (gui::Horz::None, gui::Vert::None));
        let preset_outlook = btn(&page, "Outlook", 138, 10, 68, 22, (gui::Horz::None, gui::Vert::None));
        let preset_yahoo = btn(&page, "Yahoo", 210, 10, 60, 22, (gui::Horz::None, gui::Vert::None));
        let preset_proton = btn(
            &page,
            "Proton Bridge",
            274,
            10,
            96,
            22,
            (gui::Horz::None, gui::Vert::None),
        );
        let preset_icloud = btn(&page, "iCloud", 374, 10, 60, 22, (gui::Horz::None, gui::Vert::None));
        let preset_fastmail = btn(&page, "Fastmail", 438, 10, 68, 22, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "IMAP Host:", 12, 42, 74);
        let host_edit = edit(&page, 90, 38, 210, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Port:", 310, 42, 36);
        let port_edit = edit(&page, 348, 38, 56, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Auth:", 416, 42, 40);
        let auth_combo = combo(&page, 458, 38, 150, &["password", "xoauth2", "oauthbearer"]);

        lbl(&page, "Username:", 12, 70, 74);
        let user_edit = edit(&page, 90, 66, 210, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Password:", 310, 70, 64);
        let pass_edit = edit_pass(&page, 376, 66, 160, 22);
        lbl(&page, "Token:", 546, 70, 46);
        let oauth_edit = edit_pass(&page, 594, 66, 170, 22);

        let chk_tls = chk(&page, "Use SSL/TLS", 90, 96);
        let chk_insecure = chk(&page, "Accept Self-Signed Certs", 200, 96);
        lbl(&page, "Folders:", 380, 98, 52);
        let folders_edit = edit(&page, 434, 94, 150, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Max:", 592, 98, 32);
        let max_edit = edit(&page, 626, 94, 60, 22, (gui::Horz::None, gui::Vert::None));
        let btn_connect = btn(
            &page,
            "Connect && Ingest",
            700,
            93,
            140,
            24,
            (gui::Horz::None, gui::Vert::None),
        );

        lbl(&page, "Server-to-Server IMAP Sync (Zero Disk):", 12, 134, 280);
        let sync_chk_insecure = chk(&page, "Accept Self-Signed Certs (Sync)", 310, 132);

        lbl(&page, "Src Host:", 12, 160, 60);
        let sync_src_host = edit(&page, 74, 156, 140, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Port:", 220, 160, 32);
        let sync_src_port = edit(&page, 254, 156, 44, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "User:", 304, 160, 34);
        let sync_src_user = edit(&page, 340, 156, 120, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Auth:", 466, 160, 34);
        let sync_src_auth_combo = combo(&page, 502, 156, 96, &["password", "xoauth2", "oauthbearer"]);
        lbl(&page, "Pass:", 604, 160, 34);
        let sync_src_pass = edit_pass(&page, 640, 156, 100, 22);
        lbl(&page, "Tok:", 746, 160, 28);
        let sync_src_oauth = edit_pass(&page, 776, 156, 100, 22);
        let sync_src_chk_tls = chk(&page, "Src TLS", 884, 158);

        lbl(&page, "Dst Host:", 12, 188, 60);
        let sync_dst_host = edit(&page, 74, 184, 140, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Port:", 220, 188, 32);
        let sync_dst_port = edit(&page, 254, 184, 44, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "User:", 304, 188, 34);
        let sync_dst_user = edit(&page, 340, 184, 120, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Auth:", 466, 188, 34);
        let sync_dst_auth_combo = combo(&page, 502, 184, 96, &["password", "xoauth2", "oauthbearer"]);
        lbl(&page, "Pass:", 604, 188, 34);
        let sync_dst_pass = edit_pass(&page, 640, 184, 100, 22);
        lbl(&page, "Tok:", 746, 188, 28);
        let sync_dst_oauth = edit_pass(&page, 776, 184, 100, 22);
        let sync_dst_chk_tls = chk(&page, "Dst TLS", 884, 186);

        lbl(&page, "Folders:", 12, 218, 56);
        let sync_folders = edit(&page, 72, 214, 160, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Prefix:", 240, 218, 44);
        let sync_prefix = edit(&page, 286, 214, 110, 22, (gui::Horz::None, gui::Vert::None));
        let sync_chk_flags = chk(&page, "Preserve Flags", 404, 216);
        let sync_chk_dates = chk(&page, "Preserve Dates", 516, 216);
        let sync_chk_dry_run = chk(&page, "Dry Run", 630, 216);
        let btn_sync = btn(
            &page,
            "Start IMAP Sync",
            714,
            213,
            126,
            24,
            (gui::Horz::None, gui::Vert::None),
        );

        let status_edit = readonly_multiline(&page, 12, 248, 920, 240);

        {
            let status_edit = status_edit.clone();
            page.on().wm_size(move |p| {
                if p.request == co::SIZE_R::MINIMIZED || p.client_area.cx <= 0 || p.client_area.cy <= 0 {
                    return Ok(());
                }
                let pad = gui::dpi_x(12);
                let w = (p.client_area.cx - 2 * pad).max(gui::dpi_x(260));
                let y = gui::dpi_y(248);
                let h = (p.client_area.cy - y - gui::dpi_y(10)).max(gui::dpi_y(90));
                move_ctrl(&status_edit, pad, y, w, h);
                Ok(())
            });
        }

        Self {
            page,
            preset_gmail,
            preset_outlook,
            preset_yahoo,
            preset_proton,
            preset_icloud,
            preset_fastmail,
            host_edit,
            port_edit,
            auth_combo,
            user_edit,
            pass_edit,
            oauth_edit,
            chk_tls,
            chk_insecure,
            folders_edit,
            max_edit,
            btn_connect,
            sync_src_host,
            sync_src_port,
            sync_src_user,
            sync_src_auth_combo,
            sync_src_pass,
            sync_src_oauth,
            sync_src_chk_tls,
            sync_dst_host,
            sync_dst_port,
            sync_dst_user,
            sync_dst_auth_combo,
            sync_dst_pass,
            sync_dst_oauth,
            sync_dst_chk_tls,
            sync_chk_insecure,
            sync_folders,
            sync_prefix,
            sync_chk_flags,
            sync_chk_dates,
            sync_chk_dry_run,
            btn_sync,
            status_edit,
        }
    }
}

#[derive(Clone)]
struct MergePage {
    page: gui::TabPage,
    btn_add_file: gui::Button,
    btn_add_folder: gui::Button,
    btn_add_samples: gui::Button,
    btn_clear: gui::Button,
    sources_edit: gui::Edit,
    folder_strat_combo: gui::ComboBox,
    dedup_combo: gui::ComboBox,
    format_combo: gui::ComboBox,
    chk_sort: gui::CheckBox,
    dest_edit: gui::Edit,
    btn_browse_dest: gui::Button,
    btn_merge_wb: gui::Button,
    btn_merge_file: gui::Button,
    report_edit: gui::Edit,
}

impl MergePage {
    fn new(parent: &(impl GuiParent + 'static)) -> Self {
        let page = tab_page(parent);

        lbl(&page, "Source Mailboxes:", 12, 14, 120);
        let btn_add_file = btn(
            &page,
            "Add File...",
            138,
            10,
            88,
            24,
            (gui::Horz::None, gui::Vert::None),
        );
        let btn_add_folder = btn(
            &page,
            "Add Folder...",
            232,
            10,
            96,
            24,
            (gui::Horz::None, gui::Vert::None),
        );
        let btn_add_samples = btn(
            &page,
            "Add Samples",
            334,
            10,
            96,
            24,
            (gui::Horz::None, gui::Vert::None),
        );
        let btn_clear = btn(&page, "Clear", 436, 10, 62, 24, (gui::Horz::None, gui::Vert::None));

        let sources_edit = readonly_multiline(&page, 12, 40, 920, 110);

        lbl(&page, "Folders:", 12, 162, 54);
        let folder_strat_combo = combo(&page, 70, 158, 140, &["combine", "prefix", "flatten"]);
        lbl(&page, "Dedup:", 220, 162, 46);
        let dedup_combo = combo(&page, 270, 158, 130, &["message-id", "sha256", "content", "none"]);
        lbl(&page, "Output:", 410, 162, 50);
        let format_combo = combo(&page, 464, 158, 130, &["Outlook PST", "MBOX", "EML Tree"]);
        let chk_sort = chk(&page, "Sort UTC", 604, 160);

        lbl(&page, "Destination:", 12, 194, 80);
        let dest_edit = edit(&page, 96, 190, 320, 22, (gui::Horz::None, gui::Vert::None));
        let btn_browse_dest = btn(&page, "Browse...", 422, 189, 80, 24, (gui::Horz::None, gui::Vert::None));
        let btn_merge_wb = btn(
            &page,
            "Merge into Workbench",
            514,
            189,
            150,
            24,
            (gui::Horz::None, gui::Vert::None),
        );
        let btn_merge_file = btn(
            &page,
            "Merge && Export",
            670,
            189,
            124,
            24,
            (gui::Horz::None, gui::Vert::None),
        );

        let report_edit = readonly_multiline(&page, 12, 224, 920, 260);

        {
            let sources_edit = sources_edit.clone();
            let report_edit = report_edit.clone();
            page.on().wm_size(move |p| {
                if p.request == co::SIZE_R::MINIMIZED || p.client_area.cx <= 0 || p.client_area.cy <= 0 {
                    return Ok(());
                }
                let pad = gui::dpi_x(12);
                let w = (p.client_area.cx - 2 * pad).max(gui::dpi_x(260));
                move_ctrl(&sources_edit, pad, gui::dpi_y(40), w, gui::dpi_y(110));
                let ry = gui::dpi_y(224);
                let rh = (p.client_area.cy - ry - gui::dpi_y(10)).max(gui::dpi_y(100));
                move_ctrl(&report_edit, pad, ry, w, rh);
                Ok(())
            });
        }

        Self {
            page,
            btn_add_file,
            btn_add_folder,
            btn_add_samples,
            btn_clear,
            sources_edit,
            folder_strat_combo,
            dedup_combo,
            format_combo,
            chk_sort,
            dest_edit,
            btn_browse_dest,
            btn_merge_wb,
            btn_merge_file,
            report_edit,
        }
    }
}

#[derive(Clone)]
struct ForensicsPage {
    page: gui::TabPage,
    since_edit: gui::Edit,
    until_edit: gui::Edit,
    inc_folder_edit: gui::Edit,
    exc_folder_edit: gui::Edit,
    chk_deleted: gui::CheckBox,
    btn_apply_filter: gui::Button,
    btn_reset_filter: gui::Button,

    case_edit: gui::Edit,
    examiner_edit: gui::Edit,
    bates_prefix_edit: gui::Edit,
    bates_start_edit: gui::Edit,
    manifest_edit: gui::Edit,
    btn_browse_csv: gui::Button,
    btn_stamp: gui::Button,
    btn_dry_run: gui::Button,
    result_edit: gui::Edit,
}

impl ForensicsPage {
    fn new(parent: &(impl GuiParent + 'static)) -> Self {
        let page = tab_page(parent);

        lbl(&page, "Since Date:", 12, 16, 76);
        let since_edit = edit(&page, 92, 12, 140, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Until Date:", 244, 16, 72);
        let until_edit = edit(&page, 320, 12, 140, 22, (gui::Horz::None, gui::Vert::None));
        let chk_deleted = chk(&page, "Include Deleted", 472, 14);
        let btn_apply_filter = btn(
            &page,
            "Apply Filters",
            600,
            11,
            96,
            24,
            (gui::Horz::None, gui::Vert::None),
        );
        let btn_reset_filter = btn(&page, "Reset", 702, 11, 64, 24, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "Include Globs:", 12, 48, 92);
        let inc_folder_edit = edit(&page, 108, 44, 190, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Exclude Globs:", 314, 48, 92);
        let exc_folder_edit = edit(&page, 410, 44, 220, 22, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "Case Number:", 12, 84, 92);
        let case_edit = edit(&page, 108, 80, 190, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Examiner:", 314, 84, 68);
        let examiner_edit = edit(&page, 386, 80, 190, 22, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "Bates Prefix:", 12, 116, 92);
        let bates_prefix_edit = edit(&page, 108, 112, 110, 22, (gui::Horz::None, gui::Vert::None));
        lbl(&page, "Start #:", 234, 116, 54);
        let bates_start_edit = edit(&page, 292, 112, 90, 22, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "Manifest CSV:", 12, 148, 92);
        let manifest_edit = edit(&page, 108, 144, 390, 22, (gui::Horz::None, gui::Vert::None));
        let btn_browse_csv = btn(&page, "Browse...", 504, 143, 82, 24, (gui::Horz::None, gui::Vert::None));

        let btn_stamp = btn(
            &page,
            "Execute Bates Stamping && Audit",
            108,
            178,
            220,
            26,
            (gui::Horz::None, gui::Vert::None),
        );
        let btn_dry_run = btn(
            &page,
            "Pre-Flight Dry-Run Inventory",
            338,
            178,
            200,
            26,
            (gui::Horz::None, gui::Vert::None),
        );

        let result_edit = readonly_multiline(&page, 12, 216, 920, 270);

        {
            let result_edit = result_edit.clone();
            page.on().wm_size(move |p| {
                if p.request == co::SIZE_R::MINIMIZED || p.client_area.cx <= 0 || p.client_area.cy <= 0 {
                    return Ok(());
                }
                let pad = gui::dpi_x(12);
                let w = (p.client_area.cx - 2 * pad).max(gui::dpi_x(260));
                let y = gui::dpi_y(216);
                let h = (p.client_area.cy - y - gui::dpi_y(10)).max(gui::dpi_y(100));
                move_ctrl(&result_edit, pad, y, w, h);
                Ok(())
            });
        }

        Self {
            page,
            since_edit,
            until_edit,
            inc_folder_edit,
            exc_folder_edit,
            chk_deleted,
            btn_apply_filter,
            btn_reset_filter,
            case_edit,
            examiner_edit,
            bates_prefix_edit,
            bates_start_edit,
            manifest_edit,
            btn_browse_csv,
            btn_stamp,
            btn_dry_run,
            result_edit,
        }
    }
}

#[derive(Clone)]
struct ValidatorPage {
    page: gui::TabPage,
    path_edit: gui::Edit,
    btn_browse: gui::Button,
    pwd_edit: gui::Edit,
    btn_validate: gui::Button,
    report_edit: gui::Edit,
}

impl ValidatorPage {
    fn new(parent: &(impl GuiParent + 'static)) -> Self {
        let page = tab_page(parent);

        lbl(&page, "PST / OST File:", 12, 16, 100);
        let path_edit = edit(&page, 116, 12, 460, 22, (gui::Horz::None, gui::Vert::None));
        let btn_browse = btn(&page, "Browse...", 584, 11, 84, 24, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "PST Password:", 12, 48, 100);
        let pwd_edit = edit_pass(&page, 116, 44, 220, 22);
        let btn_validate = btn(
            &page,
            "Validate [MS-PST] Container",
            350,
            43,
            210,
            24,
            (gui::Horz::None, gui::Vert::None),
        );

        let report_edit = readonly_multiline(&page, 12, 82, 920, 400);

        {
            let report_edit = report_edit.clone();
            page.on().wm_size(move |p| {
                if p.request == co::SIZE_R::MINIMIZED || p.client_area.cx <= 0 || p.client_area.cy <= 0 {
                    return Ok(());
                }
                let pad = gui::dpi_x(12);
                let w = (p.client_area.cx - 2 * pad).max(gui::dpi_x(260));
                let y = gui::dpi_y(82);
                let h = (p.client_area.cy - y - gui::dpi_y(10)).max(gui::dpi_y(100));
                move_ctrl(&report_edit, pad, y, w, h);
                Ok(())
            });
        }

        Self {
            page,
            path_edit,
            btn_browse,
            pwd_edit,
            btn_validate,
            report_edit,
        }
    }
}

#[derive(Clone)]
struct OutlookPage {
    page: gui::TabPage,
    tool_combo: gui::ComboBox,
    source_edit: gui::Edit,
    btn_browse: gui::Button,
    btn_scan: gui::Button,
    filter_edit: gui::Edit,
    btn_export: gui::Button,
    btn_mutate: gui::Button,
    list: gui::ListView,
    detail_edit: gui::Edit,
}

impl OutlookPage {
    fn new(parent: &(impl GuiParent + 'static)) -> Self {
        let page = tab_page(parent);

        lbl(&page, "Tool:", 10, 12, 36);
        let tool_combo = combo(
            &page,
            48,
            9,
            230,
            &[
                "NK2Edit (.nk2 / Stream_Autocomplete)",
                "OutlookAttachView (PST/OST/MSG/EML)",
                "OutlookStatView (Correspondent Stats)",
                "OutlookAddressBookView (Contacts/OAB)",
                "OfficeIns (Office/Outlook Add-ins)",
            ],
        );

        lbl(&page, "Source / Hive:", 286, 12, 86);
        let source_edit = edit(&page, 374, 9, 250, 22, (gui::Horz::None, gui::Vert::None));
        let btn_browse = btn(&page, "Browse...", 628, 8, 72, 24, (gui::Horz::None, gui::Vert::None));
        let btn_scan = btn(&page, "Scan / Load", 704, 8, 84, 24, (gui::Horz::None, gui::Vert::None));

        lbl(&page, "Filter:", 10, 42, 40);
        let filter_edit = edit(&page, 54, 39, 280, 22, (gui::Horz::None, gui::Vert::None));
        let btn_export = btn(
            &page,
            "Export CSV...",
            342,
            38,
            96,
            24,
            (gui::Horz::None, gui::Vert::None),
        );
        let btn_mutate = btn(
            &page,
            "Dedup NK2 / Toggle Add-in...",
            444,
            38,
            180,
            24,
            (gui::Horz::None, gui::Vert::None),
        );

        let list = make_list(
            &page,
            10,
            68,
            640,
            460,
            &[("Column", 180), ("Value", 240), ("Info", 180)],
            true,
        );
        let detail_edit = readonly_multiline(&page, 658, 68, 400, 460);

        {
            let list = list.clone();
            let detail_edit = detail_edit.clone();
            page.on().wm_size(move |p| {
                if p.request == co::SIZE_R::MINIMIZED || p.client_area.cx <= 0 || p.client_area.cy <= 0 {
                    return Ok(());
                }
                let w = p.client_area.cx;
                let h = p.client_area.cy;
                let pad = gui::dpi_x(10);
                let gap = gui::dpi_x(8);
                let top = gui::dpi_y(68);
                let body_h = (h - top - gui::dpi_y(8)).max(gui::dpi_y(120));
                let insp_w = ((w - 2 * pad) * 35 / 100).clamp(gui::dpi_x(260), gui::dpi_x(480));
                let list_w = (w - 2 * pad - gap - insp_w).max(gui::dpi_x(280));
                move_ctrl(&list, pad, top, list_w, body_h);
                move_ctrl(&detail_edit, pad + list_w + gap, top, insp_w, body_h);
                Ok(())
            });
        }

        Self {
            page,
            tool_combo,
            source_edit,
            btn_browse,
            btn_scan,
            filter_edit,
            btn_export,
            btn_mutate,
            list,
            detail_edit,
        }
    }
}

#[derive(Clone)]
struct Win32App {
    wnd: gui::WindowMain,
    path_edit: gui::Edit,
    btn_open_file: gui::Button,
    btn_open_folder: gui::Button,
    btn_ingest: gui::Button,
    btn_sample: gui::Button,
    btn_go_export: gui::Button,

    tab: gui::Tab,
    tab_pages: Vec<gui::TabPage>,
    wb_pg: WorkbenchPage,
    export_pg: ExportPage,
    imap_pg: ImapPage,
    merge_pg: MergePage,
    forensics_pg: ForensicsPage,
    val_pg: ValidatorPage,
    outlook_pg: OutlookPage,

    status_bar: gui::StatusBar,
    state: Arc<Mutex<Win32State>>,
    displayed_ids: Arc<Mutex<Vec<String>>>,
    suppress_sync: Arc<AtomicBool>,
    launch: Arc<Mutex<Option<LaunchArgs>>>,
}

fn build_main_menu() -> w::SysResult<w::HMENU> {
    let menu = w::HMENU::CreateMenu()?;

    let file_m = w::HMENU::CreatePopupMenu()?;
    file_m.append_item(&[
        w::MenuItem::Entry {
            cmd_id: IDM_OPEN_FILE,
            text: "Open Mailbox File...",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_OPEN_FOLDER,
            text: "Open Mailbox Directory...",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_LOAD_SAMPLE,
            text: "Load Sample Evaluation Data",
        },
        w::MenuItem::Separator,
        w::MenuItem::Entry {
            cmd_id: IDM_EXPORT_ALL,
            text: "Export All Messages...",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_EXPORT_FILTERED,
            text: "Export Filtered Messages...",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_EXPORT_SELECTED,
            text: "Export Selected Items...",
        },
        w::MenuItem::Separator,
        w::MenuItem::Entry {
            cmd_id: IDM_EXIT,
            text: "Exit",
        },
    ])?;

    let action_m = w::HMENU::CreatePopupMenu()?;
    action_m.append_item(&[
        w::MenuItem::Entry {
            cmd_id: IDM_TOGGLE_ROW_SEL,
            text: "Toggle Selected Row Check",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_SELECT_ALL,
            text: "Select All Visible Messages",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_INVERT_SEL,
            text: "Invert Selection",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_CLEAR_SEL,
            text: "Clear Selection",
        },
    ])?;

    let view_m = w::HMENU::CreatePopupMenu()?;
    view_m.append_item(&[
        w::MenuItem::Entry {
            cmd_id: IDM_PREV_BODY,
            text: "Preview: Message Body",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_PREV_HEADERS,
            text: "Preview: RFC 5322 Headers",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_PREV_MAPI,
            text: "Preview: MAPI Properties",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_PREV_FORENSICS,
            text: "Preview: Forensics && Hashes",
        },
        w::MenuItem::Separator,
        w::MenuItem::Entry {
            cmd_id: IDM_REFRESH,
            text: "Refresh Folders && List",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_CLEAR_SEARCH,
            text: "Clear Search Filter",
        },
    ])?;

    let tools_m = w::HMENU::CreatePopupMenu()?;
    tools_m.append_item(&[
        w::MenuItem::Entry {
            cmd_id: IDM_TAB_BASE,
            text: "Mailbox Workbench",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_TAB_BASE + 1,
            text: "Export && Auto-Split",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_TAB_BASE + 2,
            text: "Live IMAP && Sync",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_TAB_BASE + 3,
            text: "Multi-Format Merger",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_TAB_BASE + 4,
            text: "eDiscovery Forensics",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_TAB_BASE + 5,
            text: "PST Validator",
        },
        w::MenuItem::Entry {
            cmd_id: IDM_TAB_BASE + 6,
            text: "Outlook && Office",
        },
    ])?;

    let help_m = w::HMENU::CreatePopupMenu()?;
    help_m.append_item(&[
        w::MenuItem::Entry {
            cmd_id: IDM_CLEAR_DATA,
            text: "Clear Loaded Messages",
        },
        w::MenuItem::Separator,
        w::MenuItem::Entry {
            cmd_id: IDM_ABOUT,
            text: "About Mail2Rust Win32...",
        },
    ])?;

    menu.append_item(&[
        w::MenuItem::Submenu {
            submenu: &file_m,
            text: "&File",
        },
        w::MenuItem::Submenu {
            submenu: &action_m,
            text: "&Action",
        },
        w::MenuItem::Submenu {
            submenu: &view_m,
            text: "&View",
        },
        w::MenuItem::Submenu {
            submenu: &tools_m,
            text: "&Tools",
        },
        w::MenuItem::Submenu {
            submenu: &help_m,
            text: "&Help",
        },
    ])?;
    Ok(menu)
}

pub fn run(launch: LaunchArgs) -> anyhow::Result<()> {
    let hmenu = build_main_menu().map_err(|e| anyhow::anyhow!("Menu creation failed: {e}"))?;
    let state = Arc::new(Mutex::new(Win32State::default()));

    let wnd = gui::WindowMain::new(gui::WindowMainOpts {
        title: "Mail2Rust Studio (Native Win32)",
        size: gui::dpi(1140, 720),
        style: co::WS::CAPTION
            | co::WS::SYSMENU
            | co::WS::CLIPCHILDREN
            | co::WS::BORDER
            | co::WS::VISIBLE
            | co::WS::SIZEBOX
            | co::WS::MINIMIZEBOX
            | co::WS::MAXIMIZEBOX,
        class_icon: gui::Icon::Id(101),
        class_bg_brush: gui::Brush::Color(co::COLOR::BTNFACE),
        menu: hmenu,
        ..Default::default()
    });

    lbl(&wnd, "Source:", 10, 12, 50);
    let path_edit = edit(&wnd, 64, 9, 450, 22, (gui::Horz::Resize, gui::Vert::None));
    let btn_open_file = btn(
        &wnd,
        "Open File...",
        520,
        8,
        86,
        24,
        (gui::Horz::Repos, gui::Vert::None),
    );
    let btn_open_folder = btn(
        &wnd,
        "Open Folder...",
        610,
        8,
        96,
        24,
        (gui::Horz::Repos, gui::Vert::None),
    );
    let btn_ingest = btn(
        &wnd,
        "Load / Refresh",
        710,
        8,
        102,
        24,
        (gui::Horz::Repos, gui::Vert::None),
    );
    let btn_sample = btn(&wnd, "Sample Data", 816, 8, 92, 24, (gui::Horz::Repos, gui::Vert::None));
    let btn_go_export = btn(&wnd, "Export...", 914, 8, 76, 24, (gui::Horz::Repos, gui::Vert::None));

    let wb_pg = WorkbenchPage::new(&wnd);
    let export_pg = ExportPage::new(&wnd);
    let imap_pg = ImapPage::new(&wnd);
    let merge_pg = MergePage::new(&wnd);
    let forensics_pg = ForensicsPage::new(&wnd);
    let val_pg = ValidatorPage::new(&wnd);
    let outlook_pg = OutlookPage::new(&wnd);

    let tab_pages = vec![
        wb_pg.page.clone(),
        export_pg.page.clone(),
        imap_pg.page.clone(),
        merge_pg.page.clone(),
        forensics_pg.page.clone(),
        val_pg.page.clone(),
        outlook_pg.page.clone(),
    ];

    let tab = gui::Tab::new(
        &wnd,
        gui::TabOpts {
            position: gui::dpi(10, 40),
            size: gui::dpi(1120, 640),
            resize_behavior: (gui::Horz::Resize, gui::Vert::Resize),
            pages: &[
                ("Mailbox Workbench", wb_pg.page.clone()),
                ("Export && Auto-Split", export_pg.page.clone()),
                ("Live IMAP && Sync", imap_pg.page.clone()),
                ("Multi-Format Merger", merge_pg.page.clone()),
                ("eDiscovery Forensics", forensics_pg.page.clone()),
                ("PST Validator", val_pg.page.clone()),
                ("Outlook && Office", outlook_pg.page.clone()),
            ],
            ..Default::default()
        },
    );

    let status_bar = gui::StatusBar::new(
        &wnd,
        &[
            gui::SbPart::Proportional(2),
            gui::SbPart::Fixed(gui::dpi_x(165)),
            gui::SbPart::Fixed(gui::dpi_x(180)),
            gui::SbPart::Fixed(gui::dpi_x(210)),
        ],
    );

    let app = Win32App {
        wnd,
        path_edit,
        btn_open_file,
        btn_open_folder,
        btn_ingest,
        btn_sample,
        btn_go_export,
        tab,
        tab_pages,
        wb_pg,
        export_pg,
        imap_pg,
        merge_pg,
        forensics_pg,
        val_pg,
        outlook_pg,
        status_bar,
        state,
        displayed_ids: Arc::new(Mutex::new(Vec::new())),
        suppress_sync: Arc::new(AtomicBool::new(false)),
        launch: Arc::new(Mutex::new(Some(launch))),
    };

    app.register_events();
    app.wnd
        .run_main(None)
        .map_err(|e| anyhow::anyhow!("Win32 message loop error: {e}"))?;
    Ok(())
}

impl Win32App {
    fn select_tab_index(&self, idx: u32) {
        unsafe {
            self.tab.hwnd().SendMessage(msg::TcmSetCurSel { index: idx });
        }
        self.layout_active_tab_page(idx);
    }

    fn layout_active_tab_page(&self, active_idx: u32) {
        for (i, pg) in self.tab_pages.iter().enumerate() {
            if i as u32 != active_idx {
                let _ = pg.hwnd().ShowWindow(co::SW::HIDE);
            }
        }
        if let Some(pg) = self.tab_pages.get(active_idx as usize)
            && let Ok(tab_rc) = self.tab.hwnd().GetWindowRect()
            && let Ok(mut rc) = self.wnd.hwnd().ScreenToClientRc(tab_rc)
        {
            unsafe {
                self.tab.hwnd().SendMessage(msg::TcmAdjustRect {
                    display_rect: false,
                    rect: &mut rc,
                });
            }
            let _ = pg.hwnd().SetWindowPos(
                w::HwndPlace::None,
                w::POINT::with(rc.left, rc.top),
                w::SIZE::with((rc.right - rc.left).max(10), (rc.bottom - rc.top).max(10)),
                co::SWP::NOZORDER | co::SWP::SHOWWINDOW,
            );
        }
    }

    fn sync_controls_to_state(&self) {
        if self.suppress_sync.load(Ordering::SeqCst) {
            return;
        }
        let mut st = self.state.lock().unwrap();
        st.open_mailbox_path = self.path_edit.text().unwrap_or_default();
        st.search_query = self.wb_pg.search_edit.text().unwrap_or_default();

        // Export & Split
        st.export_scope = ExportScope::from_index(self.export_pg.scope_combo.items().selected_index());
        st.export_format = ExportFormat::from_index(self.export_pg.format_combo.items().selected_index());
        st.pst_dest_path = self.export_pg.dest_edit.text().unwrap_or_default();
        st.pst_root = self.export_pg.root_edit.text().unwrap_or_default();
        if let Ok(gb) = self
            .export_pg
            .max_gb_edit
            .text()
            .unwrap_or_default()
            .trim()
            .parse::<usize>()
        {
            st.pst_max_gb = gb.clamp(1, 50);
        }
        st.pst_cluster_threads = self.export_pg.chk_cluster_threads.is_checked();
        st.pst_encryption = match self.export_pg.enc_combo.items().selected_index() {
            Some(1) => PstEncryption::Cyclic,
            Some(2) => PstEncryption::None,
            _ => PstEncryption::Permute,
        };
        st.pst_password = self.export_pg.pwd_edit.text().unwrap_or_default();

        // IMAP Connect
        st.imap_host = self.imap_pg.host_edit.text().unwrap_or_default();
        st.imap_port = self.imap_pg.port_edit.text().unwrap_or_default();
        st.imap_auth_method = match self.imap_pg.auth_combo.items().selected_index() {
            Some(1) => "xoauth2".to_string(),
            Some(2) => "oauthbearer".to_string(),
            _ => "password".to_string(),
        };
        st.imap_username = self.imap_pg.user_edit.text().unwrap_or_default();
        st.imap_password = self.imap_pg.pass_edit.text().unwrap_or_default();
        st.imap_oauth_token = self.imap_pg.oauth_edit.text().unwrap_or_default();
        st.imap_use_tls = self.imap_pg.chk_tls.is_checked();
        st.imap_insecure_certs = self.imap_pg.chk_insecure.is_checked();
        st.imap_folder_filter = self.imap_pg.folders_edit.text().unwrap_or_default();
        st.imap_max_per_folder = self.imap_pg.max_edit.text().unwrap_or_default();

        // IMAP Sync
        st.imap_sync_src_host = self.imap_pg.sync_src_host.text().unwrap_or_default();
        st.imap_sync_src_port = self.imap_pg.sync_src_port.text().unwrap_or_default();
        st.imap_sync_src_user = self.imap_pg.sync_src_user.text().unwrap_or_default();
        st.imap_sync_src_auth_method = match self.imap_pg.sync_src_auth_combo.items().selected_index() {
            Some(1) => "xoauth2".to_string(),
            Some(2) => "oauthbearer".to_string(),
            _ => "password".to_string(),
        };
        st.imap_sync_src_pass = self.imap_pg.sync_src_pass.text().unwrap_or_default();
        st.imap_sync_src_oauth_token = self.imap_pg.sync_src_oauth.text().unwrap_or_default();
        st.imap_sync_src_tls = self.imap_pg.sync_src_chk_tls.is_checked();
        st.imap_sync_dst_host = self.imap_pg.sync_dst_host.text().unwrap_or_default();
        st.imap_sync_dst_port = self.imap_pg.sync_dst_port.text().unwrap_or_default();
        st.imap_sync_dst_user = self.imap_pg.sync_dst_user.text().unwrap_or_default();
        st.imap_sync_dst_auth_method = match self.imap_pg.sync_dst_auth_combo.items().selected_index() {
            Some(1) => "xoauth2".to_string(),
            Some(2) => "oauthbearer".to_string(),
            _ => "password".to_string(),
        };
        st.imap_sync_dst_pass = self.imap_pg.sync_dst_pass.text().unwrap_or_default();
        st.imap_sync_dst_oauth_token = self.imap_pg.sync_dst_oauth.text().unwrap_or_default();
        st.imap_sync_dst_tls = self.imap_pg.sync_dst_chk_tls.is_checked();
        st.imap_sync_insecure = self.imap_pg.sync_chk_insecure.is_checked();
        st.imap_sync_folders = self.imap_pg.sync_folders.text().unwrap_or_default();
        st.imap_sync_folder_prefix = self.imap_pg.sync_prefix.text().unwrap_or_default();
        st.imap_sync_preserve_flags = self.imap_pg.sync_chk_flags.is_checked();
        st.imap_sync_preserve_dates = self.imap_pg.sync_chk_dates.is_checked();
        st.imap_sync_dry_run = self.imap_pg.sync_chk_dry_run.is_checked();

        // Merger
        st.merge_folder_strategy = match self.merge_pg.folder_strat_combo.items().selected_index() {
            Some(1) => "prefix".to_string(),
            Some(2) => "flatten".to_string(),
            _ => "combine".to_string(),
        };
        st.merge_dedup_strategy = match self.merge_pg.dedup_combo.items().selected_index() {
            Some(1) => "sha256".to_string(),
            Some(2) => "content".to_string(),
            Some(3) => "none".to_string(),
            _ => "message-id".to_string(),
        };
        st.merge_export_format = ExportFormat::from_index(self.merge_pg.format_combo.items().selected_index());
        st.merge_sort_by_date = self.merge_pg.chk_sort.is_checked();
        st.merge_dest_path = self.merge_pg.dest_edit.text().unwrap_or_default();

        // Forensics & Filters
        st.filter_since = self.forensics_pg.since_edit.text().unwrap_or_default();
        st.filter_until = self.forensics_pg.until_edit.text().unwrap_or_default();
        st.filter_include_folder = self.forensics_pg.inc_folder_edit.text().unwrap_or_default();
        st.filter_exclude_folder = self.forensics_pg.exc_folder_edit.text().unwrap_or_default();
        st.filter_include_deleted = self.forensics_pg.chk_deleted.is_checked();
        st.case_number = self.forensics_pg.case_edit.text().unwrap_or_default();
        st.examiner = self.forensics_pg.examiner_edit.text().unwrap_or_default();
        st.bates_prefix = self.forensics_pg.bates_prefix_edit.text().unwrap_or_default();
        if let Ok(start) = self
            .forensics_pg
            .bates_start_edit
            .text()
            .unwrap_or_default()
            .trim()
            .parse::<usize>()
        {
            st.bates_start = start;
        }
        st.manifest_csv_path = self.forensics_pg.manifest_edit.text().unwrap_or_default();

        // Validator
        st.validate_pst_path = self.val_pg.path_edit.text().unwrap_or_default();
        st.validate_pst_password = self.val_pg.pwd_edit.text().unwrap_or_default();
    }

    fn populate_controls_from_state(&self) {
        self.suppress_sync.store(true, Ordering::SeqCst);
        let st = self.state.lock().unwrap();
        let _ = self.path_edit.set_text(&st.open_mailbox_path);
        let _ = self.wb_pg.search_edit.set_text(&st.search_query);

        let _ = self.export_pg.dest_edit.set_text(&st.pst_dest_path);
        let _ = self.export_pg.root_edit.set_text(&st.pst_root);
        let _ = self.export_pg.max_gb_edit.set_text(&st.pst_max_gb.to_string());
        self.export_pg.chk_cluster_threads.set_check(st.pst_cluster_threads);

        let _ = self.imap_pg.host_edit.set_text(&st.imap_host);
        let _ = self.imap_pg.port_edit.set_text(&st.imap_port);
        self.imap_pg
            .auth_combo
            .items()
            .select(Some(match st.imap_auth_method.to_ascii_lowercase().as_str() {
                "xoauth2" => 1,
                "oauthbearer" => 2,
                _ => 0,
            }));
        let _ = self.imap_pg.user_edit.set_text(&st.imap_username);
        let _ = self.imap_pg.oauth_edit.set_text(&st.imap_oauth_token);
        self.imap_pg.chk_tls.set_check(st.imap_use_tls);
        self.imap_pg.chk_insecure.set_check(st.imap_insecure_certs);
        let _ = self.imap_pg.sync_src_host.set_text(&st.imap_sync_src_host);
        let _ = self.imap_pg.sync_src_port.set_text(&st.imap_sync_src_port);
        self.imap_pg.sync_src_auth_combo.items().select(Some(
            match st.imap_sync_src_auth_method.to_ascii_lowercase().as_str() {
                "xoauth2" => 1,
                "oauthbearer" => 2,
                _ => 0,
            },
        ));
        let _ = self.imap_pg.sync_src_oauth.set_text(&st.imap_sync_src_oauth_token);
        self.imap_pg.sync_src_chk_tls.set_check(st.imap_sync_src_tls);
        let _ = self.imap_pg.sync_dst_host.set_text(&st.imap_sync_dst_host);
        let _ = self.imap_pg.sync_dst_port.set_text(&st.imap_sync_dst_port);
        self.imap_pg.sync_dst_auth_combo.items().select(Some(
            match st.imap_sync_dst_auth_method.to_ascii_lowercase().as_str() {
                "xoauth2" => 1,
                "oauthbearer" => 2,
                _ => 0,
            },
        ));
        let _ = self.imap_pg.sync_dst_oauth.set_text(&st.imap_sync_dst_oauth_token);
        self.imap_pg.sync_dst_chk_tls.set_check(st.imap_sync_dst_tls);
        self.imap_pg.sync_chk_insecure.set_check(st.imap_sync_insecure);
        self.imap_pg.sync_chk_flags.set_check(st.imap_sync_preserve_flags);
        self.imap_pg.sync_chk_dates.set_check(st.imap_sync_preserve_dates);
        self.imap_pg.sync_chk_dry_run.set_check(st.imap_sync_dry_run);

        self.merge_pg.chk_sort.set_check(st.merge_sort_by_date);
        let _ = self.merge_pg.dest_edit.set_text(&st.merge_dest_path);

        let _ = self.forensics_pg.since_edit.set_text(&st.filter_since);
        let _ = self.forensics_pg.until_edit.set_text(&st.filter_until);
        let _ = self.forensics_pg.inc_folder_edit.set_text(&st.filter_include_folder);
        let _ = self.forensics_pg.exc_folder_edit.set_text(&st.filter_exclude_folder);
        self.forensics_pg.chk_deleted.set_check(st.filter_include_deleted);
        let _ = self.forensics_pg.bates_prefix_edit.set_text(&st.bates_prefix);
        let _ = self.forensics_pg.bates_start_edit.set_text(&st.bates_start.to_string());
        self.suppress_sync.store(false, Ordering::SeqCst);
    }

    fn refresh_status_bar(&self, st: &Win32State) {
        let filtered_count = st.filtered_messages().len();
        let _ = self.status_bar.parts().get(0).set_text(&st.status_message);
        let _ = self
            .status_bar
            .parts()
            .get(1)
            .set_text(&format!("Folder: {}", st.active_folder));
        let _ = self.status_bar.parts().get(2).set_text(&format!(
            "Shown: {filtered_count}/{} (Sel: {})",
            st.messages.len(),
            st.selected_message_ids.len()
        ));
        let _ = self
            .status_bar
            .parts()
            .get(3)
            .set_text("100% Pure Rust - Zero Outlook COM");
    }

    fn refresh_folders_combo(&self) {
        let mut names = vec!["All Mail".to_string()];
        let st = self.state.lock().unwrap();
        for f in &st.folders {
            let label = if f.unread_count > 0 {
                format!("{} ({}/{})", f.name, f.unread_count, f.message_count)
            } else {
                format!("{} ({})", f.name, f.message_count)
            };
            names.push(label);
        }
        let sel_idx = if st.active_folder == "All Mail" {
            0
        } else {
            st.folders
                .iter()
                .position(|f| f.name == st.active_folder)
                .map(|i| i + 1)
                .unwrap_or(0)
        } as u32;
        drop(st);

        self.suppress_sync.store(true, Ordering::SeqCst);
        self.wb_pg.folder_combo.items().delete_all();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let _ = self.wb_pg.folder_combo.items().add(&refs);
        self.wb_pg.folder_combo.items().select(Some(sel_idx));
        self.suppress_sync.store(false, Ordering::SeqCst);
    }

    fn format_message_preview(msg: &EmailMessage, tab: PreviewTab) -> String {
        let raw = match tab {
            PreviewTab::Body => {
                let to_str = msg
                    .to
                    .iter()
                    .map(|r| format!("{} <{}>", r.name, r.email))
                    .collect::<Vec<_>>()
                    .join("; ");
                let mut out = format!(
                    "Subject: {}\nFrom: {} <{}>\nDate: {}\n",
                    msg.subject,
                    msg.from.name,
                    msg.from.email,
                    msg.date.format("%Y-%m-%d %H:%M:%S UTC"),
                );
                if !to_str.is_empty() {
                    out.push_str(&format!("To: {to_str}\n"));
                }
                if !msg.attachments.is_empty() {
                    out.push_str("Attachments:\n");
                    for att in &msg.attachments {
                        out.push_str(&format!("  - {} ({})\n", att.filename, format_size(att.size_bytes)));
                    }
                }
                out.push_str("\n------------------------------------------------------------\n\n");
                out.push_str(&msg.body_text);
                out
            }
            PreviewTab::Headers => format!(
                "Message-ID: {}\nDate: {}\nFrom: {} <{}>\nSubject: {}\nSource-Format: {}\nConversation-ID: {}",
                msg.message_id,
                msg.date.to_rfc2822(),
                msg.from.name,
                msg.from.email,
                msg.subject,
                msg.source_format,
                msg.conversation_id
            ),
            PreviewTab::Mapi => format!(
                "Outlook [MS-OXPROPS] MAPI DASL Mapping:\n\
                 - 0x0037001F PR_SUBJECT (PT_UNICODE): {}\n\
                 - 0x0E070003 PR_MESSAGE_FLAGS (PT_LONG): {}\n\
                 - 0x30070040 PR_CREATION_TIME (PT_SYSTIME): {}\n\
                 - 0x30130102 PR_CONVERSATION_ID (PT_BINARY): {}",
                msg.subject,
                if msg.flags.read {
                    "0x0001 (MSGFLAG_READ)"
                } else {
                    "0x0000"
                },
                msg.date.format("%Y-%m-%d %H:%M:%S UTC"),
                msg.conversation_id
            ),
            PreviewTab::Forensics => format!(
                "Chain-of-Custody & Integrity Verification:\n\
                 - SHA-256: {}\n\
                 - MD5: {}\n\
                 - Bates Number: {}\n\
                 - Legal Hold: {}\n\
                 - Size: {} bytes",
                msg.sha256,
                msg.md5,
                msg.bates_number.as_deref().unwrap_or("Unassigned"),
                if msg.legal_hold {
                    "ACTIVE (Tamper-Evident)"
                } else {
                    "Inactive"
                },
                msg.size_bytes
            ),
        };
        to_crlf(&raw)
    }

    fn refresh_preview(&self) {
        let st = self.state.lock().unwrap();
        let text = match st.get_selected_message() {
            Some(msg) => Self::format_message_preview(msg, st.preview_tab),
            None => "Select a message from the list to inspect its body, headers, MAPI tags, and hashes.".to_string(),
        };
        let _ = self.wb_pg.preview_edit.set_text(&text);
    }

    fn refresh_message_list(&self) {
        let st = self.state.lock().unwrap();
        let filtered = st.filtered_messages();
        let mut ids = Vec::with_capacity(filtered.len());

        self.wb_pg.list.set_redraw(false);
        let _ = self.wb_pg.list.items().delete_all();

        for msg in filtered {
            ids.push(msg.id.clone());
            let bates_or_id = msg.bates_number.as_deref().unwrap_or(&msg.id);
            let imp = match msg.importance {
                mail2rust_core::Importance::High => "!",
                mail2rust_core::Importance::Low => "v",
                mail2rust_core::Importance::Normal => "",
            };
            let flags = format!(
                "{}{}{}{}",
                if st.selected_message_ids.contains(&msg.id) {
                    "[X] "
                } else {
                    ""
                },
                if !msg.flags.read { "U" } else { "-" },
                if !msg.attachments.is_empty() { "@" } else { "-" },
                if msg.flags.flagged { "*" } else { "-" },
            );
            let from = if msg.from.name.is_empty() {
                msg.from.email.clone()
            } else {
                format!("{} <{}>", msg.from.name, msg.from.email)
            };
            let subj = if msg.subject.is_empty() {
                "(No Subject)"
            } else {
                &msg.subject
            };
            let date_str = msg.date.format("%Y-%m-%d %H:%M").to_string();
            let size_str = format_size(msg.size_bytes);
            let _ =
                self.wb_pg
                    .list
                    .items()
                    .add(&[bates_or_id, imp, &flags, &from, subj, &date_str, &size_str], None, ());
        }
        self.wb_pg.list.set_redraw(true);
        *self.displayed_ids.lock().unwrap() = ids;
        self.refresh_status_bar(&st);
        drop(st);
        self.refresh_preview();
    }

    fn refresh_outlook_grid(&self, rebuild_cols: bool) {
        let st = self.state.lock().unwrap();
        self.outlook_pg.list.set_redraw(false);
        let _ = self.outlook_pg.list.items().delete_all();

        if rebuild_cols {
            loop {
                let count = self.outlook_pg.list.cols().count().unwrap_or(0);
                if count == 0 {
                    break;
                }
                if unsafe {
                    self.outlook_pg
                        .list
                        .hwnd()
                        .SendMessage(msg::LvmDeleteColumn { index: 0 })
                }
                .is_err()
                {
                    break;
                }
            }
            for col in st.outlook_grid.columns() {
                let w = gui::dpi_x(i32::from(col.width_hint.max(70)));
                let _ = self.outlook_pg.list.cols().add(&col.label, w);
            }
        }

        let col_count = st.outlook_grid.columns().len();
        let row_count = st.outlook_grid.row_count();
        for r in 0..row_count {
            let cells: Vec<String> = (0..col_count).map(|c| st.outlook_grid.cell_text(r, c)).collect();
            let refs: Vec<&str> = cells.iter().map(String::as_str).collect();
            let _ = self.outlook_pg.list.items().add(&refs, None, ());
        }
        self.outlook_pg.list.set_redraw(true);
        let detail = st.outlook_grid.format_inspector_crlf();
        self.refresh_status_bar(&st);
        drop(st);
        let _ = self.outlook_pg.detail_edit.set_text(&detail);
    }

    fn refresh_all_views(&self) {
        self.refresh_folders_combo();
        self.refresh_message_list();
    }

    fn apply_imap_preset(&self, host: &str, port: &str, tls: bool, insecure: bool) {
        self.suppress_sync.store(true, Ordering::SeqCst);
        let _ = self.imap_pg.host_edit.set_text(host);
        let _ = self.imap_pg.port_edit.set_text(port);
        self.imap_pg.chk_tls.set_check(tls);
        self.imap_pg.chk_insecure.set_check(insecure);
        self.suppress_sync.store(false, Ordering::SeqCst);
        self.sync_controls_to_state();
    }

    fn open_mailbox_file_dialog(&self) {
        let initial = self.state.lock().unwrap().open_mailbox_path.clone();
        if let Ok(Some(picked)) =
            pick_open_file(self.wnd.hwnd(), "Open Mailbox File", MAIL_OPEN_FILTERS, Some(&initial))
        {
            {
                let mut st = self.state.lock().unwrap();
                st.open_mailbox_path = picked.to_string_lossy().to_string();
                st.execute_open_mailbox();
            }
            let _ = self.path_edit.set_text(&picked.to_string_lossy());
            self.refresh_all_views();
        }
    }

    fn open_mailbox_folder_dialog(&self) {
        let initial = self.state.lock().unwrap().open_mailbox_path.clone();
        if let Ok(Some(picked)) = pick_folder(
            self.wnd.hwnd(),
            "Open Mailbox Directory (Maildir, Thunderbird Profile, EML Tree)",
            Some(&initial),
        ) {
            {
                let mut st = self.state.lock().unwrap();
                st.open_mailbox_path = picked.to_string_lossy().to_string();
                st.execute_open_mailbox();
            }
            let _ = self.path_edit.set_text(&picked.to_string_lossy());
            self.refresh_all_views();
        }
    }

    fn register_events(&self) {
        let self2 = self.clone();
        self.wnd.on().wm_create(move |_| {
            self2.populate_controls_from_state();
            self2.layout_active_tab_page(0);

            let mut should_pick = false;
            if let Some(launch) = self2.launch.lock().unwrap().take() {
                if let Some(p) = launch.path {
                    let mut st = self2.state.lock().unwrap();
                    st.open_mailbox_path = p.to_string_lossy().to_string();
                    st.execute_open_mailbox();
                } else if launch.sample {
                    self2.state.lock().unwrap().load_sample_data();
                } else if launch.pick {
                    should_pick = true;
                }
            }
            self2.populate_controls_from_state();
            self2.refresh_all_views();
            if should_pick {
                self2.open_mailbox_file_dialog();
            }
            Ok(0)
        });

        let self2 = self.clone();
        self.wnd.on().wm_size(move |_| {
            let idx = self2.tab.items().selected().map(|i| i.index()).unwrap_or(0);
            self2.layout_active_tab_page(idx);
            Ok(())
        });

        let self2 = self.clone();
        self.tab.on().tcn_sel_change(move || {
            let idx = self2.tab.items().selected().map(|i| i.index()).unwrap_or(0);
            self2.layout_active_tab_page(idx);
            Ok(())
        });

        // Top bar actions
        {
            let app = self.clone();
            self.path_edit.on().en_change(move || {
                app.sync_controls_to_state();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.btn_open_file.on().bn_clicked(move || {
                app.open_mailbox_file_dialog();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.btn_open_folder.on().bn_clicked(move || {
                app.open_mailbox_folder_dialog();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.btn_ingest.on().bn_clicked(move || {
                app.sync_controls_to_state();
                app.state.lock().unwrap().execute_open_mailbox();
                app.refresh_all_views();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.btn_sample.on().bn_clicked(move || {
                app.state.lock().unwrap().load_sample_data();
                app.refresh_all_views();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.btn_go_export.on().bn_clicked(move || {
                app.select_tab_index(1);
                Ok(())
            });
        }

        // Workbench events
        {
            let app = self.clone();
            self.wb_pg.folder_combo.on().cbn_sel_change(move || {
                if app.suppress_sync.load(Ordering::SeqCst) {
                    return Ok(());
                }
                let sel_idx = app.wb_pg.folder_combo.items().selected_index().unwrap_or(0) as usize;
                {
                    let mut st = app.state.lock().unwrap();
                    st.active_folder = if sel_idx == 0 {
                        "All Mail".to_string()
                    } else {
                        st.folders
                            .get(sel_idx - 1)
                            .map(|f| f.name.clone())
                            .unwrap_or_else(|| "All Mail".to_string())
                    };
                }
                app.refresh_message_list();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.wb_pg.search_edit.on().en_change(move || {
                if app.suppress_sync.load(Ordering::SeqCst) {
                    return Ok(());
                }
                app.sync_controls_to_state();
                app.refresh_message_list();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.wb_pg.btn_clear_search.on().bn_clicked(move || {
                let _ = app.wb_pg.search_edit.set_text("");
                app.state.lock().unwrap().search_query.clear();
                app.refresh_message_list();
                Ok(())
            });
        }
        for (button, ptab) in [
            (&self.wb_pg.btn_prev_body, PreviewTab::Body),
            (&self.wb_pg.btn_prev_hdr, PreviewTab::Headers),
            (&self.wb_pg.btn_prev_mapi, PreviewTab::Mapi),
            (&self.wb_pg.btn_prev_for, PreviewTab::Forensics),
        ] {
            let app = self.clone();
            button.on().bn_clicked(move || {
                app.state.lock().unwrap().preview_tab = ptab;
                app.refresh_preview();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.wb_pg.btn_copy_preview.on().bn_clicked(move || {
                let txt = app.wb_pg.preview_edit.text().unwrap_or_default();
                if copy_to_clipboard(&txt) {
                    let mut st = app.state.lock().unwrap();
                    st.status_message = "Copied preview text to clipboard.".to_string();
                    app.refresh_status_bar(&st);
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.wb_pg.btn_toggle_sel.on().bn_clicked(move || {
                let mut st = app.state.lock().unwrap();
                if let Some(id) = st.selected_message_id.clone()
                    && !st.selected_message_ids.remove(&id)
                {
                    st.selected_message_ids.insert(id);
                }
                drop(st);
                app.refresh_message_list();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.wb_pg.list.on().lvn_item_changed(move |p| {
                if p.uNewState.has(co::LVIS::SELECTED) && p.iItem >= 0 {
                    let ids = app.displayed_ids.lock().unwrap();
                    if let Some(id) = ids.get(p.iItem as usize).cloned() {
                        app.state.lock().unwrap().selected_message_id = Some(id);
                        drop(ids);
                        app.refresh_preview();
                    }
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.wb_pg.list.on().lvn_column_click(move |p| {
                let col = match p.iSubItem {
                    1 => SortColumn::Importance,
                    2 => SortColumn::Flags,
                    3 => SortColumn::From,
                    4 => SortColumn::Subject,
                    5 => SortColumn::Date,
                    6 => SortColumn::Size,
                    _ => SortColumn::Date,
                };
                app.state.lock().unwrap().toggle_sort(col);
                app.refresh_message_list();
                Ok(())
            });
        }

        // Export & Split page
        {
            let app = self.clone();
            self.export_pg.btn_browse_dest.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let fmt = app.state.lock().unwrap().export_format;
                let picked = match fmt {
                    ExportFormat::Pst => pick_save_file(
                        app.wnd.hwnd(),
                        "Select Destination Outlook PST File",
                        "Export.pst",
                        "pst",
                        &[("Outlook PST (*.pst)", "*.pst"), ("All Files", "*.*")],
                    ),
                    ExportFormat::Mbox => pick_save_file(
                        app.wnd.hwnd(),
                        "Select Destination MBOX File",
                        "Export.mbox",
                        "mbox",
                        &[("MBOX Mailbox (*.mbox)", "*.mbox"), ("All Files", "*.*")],
                    ),
                    ExportFormat::Eml => pick_folder(app.wnd.hwnd(), "Select Destination EML Directory", None),
                };
                if let Ok(Some(p)) = picked {
                    let s = p.to_string_lossy().to_string();
                    let _ = app.export_pg.dest_edit.set_text(&s);
                    app.state.lock().unwrap().pst_dest_path = s;
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.export_pg.btn_export.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let mut st = app.state.lock().unwrap();
                st.execute_export();
                let txt = to_crlf(st.pst_status_message.as_deref().unwrap_or(""));
                app.refresh_status_bar(&st);
                drop(st);
                let _ = app.export_pg.status_edit.set_text(&txt);
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.export_pg.btn_auto_split.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let mut st = app.state.lock().unwrap();
                st.execute_auto_split();
                let txt = to_crlf(st.pst_status_message.as_deref().unwrap_or(""));
                app.refresh_status_bar(&st);
                drop(st);
                let _ = app.export_pg.status_edit.set_text(&txt);
                Ok(())
            });
        }

        // IMAP page
        for (button, host, port, insecure) in [
            (&self.imap_pg.preset_gmail, "imap.gmail.com", "993", false),
            (&self.imap_pg.preset_outlook, "outlook.office365.com", "993", false),
            (&self.imap_pg.preset_yahoo, "imap.mail.yahoo.com", "993", false),
            (&self.imap_pg.preset_proton, "127.0.0.1", "1143", true),
            (&self.imap_pg.preset_icloud, "imap.mail.me.com", "993", false),
            (&self.imap_pg.preset_fastmail, "imap.fastmail.com", "993", false),
        ] {
            let app = self.clone();
            button.on().bn_clicked(move || {
                app.apply_imap_preset(host, port, true, insecure);
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.imap_pg.btn_connect.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let msg = {
                    let mut st = app.state.lock().unwrap();
                    st.execute_imap_connect();
                    to_crlf(st.imap_status.as_deref().unwrap_or(""))
                };
                let _ = app.imap_pg.status_edit.set_text(&msg);
                app.refresh_all_views();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.imap_pg.btn_sync.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let mut st = app.state.lock().unwrap();
                st.execute_imap_sync();
                let msg = to_crlf(st.imap_sync_status.as_deref().unwrap_or(""));
                app.refresh_status_bar(&st);
                drop(st);
                let _ = app.imap_pg.status_edit.set_text(&msg);
                Ok(())
            });
        }

        // Merge page
        {
            let app = self.clone();
            self.merge_pg.btn_add_file.on().bn_clicked(move || {
                if let Ok(paths) =
                    pick_open_files(app.wnd.hwnd(), "Add Source Mailbox File(s)", MAIL_OPEN_FILTERS, None)
                    && !paths.is_empty()
                {
                    let mut st = app.state.lock().unwrap();
                    for p in paths {
                        let s = p.to_string_lossy().to_string();
                        if !st.merge_source_paths.contains(&s) {
                            st.merge_source_paths.push(s);
                        }
                    }
                    let txt = st.merge_source_paths.join("\r\n");
                    drop(st);
                    let _ = app.merge_pg.sources_edit.set_text(&txt);
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.merge_pg.btn_add_folder.on().bn_clicked(move || {
                if let Ok(Some(p)) = pick_folder(app.wnd.hwnd(), "Add Source Mailbox Directory", None) {
                    let mut st = app.state.lock().unwrap();
                    let s = p.to_string_lossy().to_string();
                    if !st.merge_source_paths.contains(&s) {
                        st.merge_source_paths.push(s);
                    }
                    let txt = st.merge_source_paths.join("\r\n");
                    drop(st);
                    let _ = app.merge_pg.sources_edit.set_text(&txt);
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.merge_pg.btn_add_samples.on().bn_clicked(move || {
                let mut st = app.state.lock().unwrap();
                for cand in [
                    "samples/takeout/sample_google_takeout.zip",
                    "samples/maildir",
                    "samples/emlx/sample_apple_mail.emlx",
                    "samples/contacts/sample_contacts.vcf",
                    "samples/calendar/sample_calendar.ics",
                    "samples/mbox/sample_mailbox.mbox",
                ] {
                    if Path::new(cand).exists() && !st.merge_source_paths.iter().any(|x| x == cand) {
                        st.merge_source_paths.push(cand.to_string());
                    }
                }
                let txt = if st.merge_source_paths.is_empty() {
                    "(No ./samples directory found in current working directory)".to_string()
                } else {
                    st.merge_source_paths.join("\r\n")
                };
                drop(st);
                let _ = app.merge_pg.sources_edit.set_text(&txt);
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.merge_pg.btn_clear.on().bn_clicked(move || {
                app.state.lock().unwrap().merge_source_paths.clear();
                let _ = app.merge_pg.sources_edit.set_text("");
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.merge_pg.btn_browse_dest.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let fmt = app.state.lock().unwrap().merge_export_format;
                let picked = match fmt {
                    ExportFormat::Pst => pick_save_file(
                        app.wnd.hwnd(),
                        "Select Merged PST Target",
                        "merged_archive.pst",
                        "pst",
                        &[("Outlook PST (*.pst)", "*.pst"), ("All Files", "*.*")],
                    ),
                    ExportFormat::Mbox => pick_save_file(
                        app.wnd.hwnd(),
                        "Select Merged MBOX Target",
                        "merged_archive.mbox",
                        "mbox",
                        &[("MBOX (*.mbox)", "*.mbox"), ("All Files", "*.*")],
                    ),
                    ExportFormat::Eml => pick_folder(app.wnd.hwnd(), "Select Merged EML Directory", None),
                };
                if let Ok(Some(p)) = picked {
                    let s = p.to_string_lossy().to_string();
                    let _ = app.merge_pg.dest_edit.set_text(&s);
                    app.state.lock().unwrap().merge_dest_path = s;
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.merge_pg.btn_merge_wb.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let report = {
                    let mut st = app.state.lock().unwrap();
                    st.execute_merge_to_workbench();
                    to_crlf(&format!(
                        "{}\n\n{}",
                        st.merge_status.as_deref().unwrap_or(""),
                        st.merge_report_text.as_deref().unwrap_or("")
                    ))
                };
                let _ = app.merge_pg.report_edit.set_text(&report);
                app.refresh_all_views();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.merge_pg.btn_merge_file.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let mut st = app.state.lock().unwrap();
                st.execute_merge_to_file();
                let report = to_crlf(&format!(
                    "{}\n\n{}",
                    st.merge_status.as_deref().unwrap_or(""),
                    st.merge_report_text.as_deref().unwrap_or("")
                ));
                app.refresh_status_bar(&st);
                drop(st);
                let _ = app.merge_pg.report_edit.set_text(&report);
                Ok(())
            });
        }

        // Forensics & Filter page
        {
            let app = self.clone();
            self.forensics_pg.btn_apply_filter.on().bn_clicked(move || {
                app.sync_controls_to_state();
                app.refresh_message_list();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.forensics_pg.btn_reset_filter.on().bn_clicked(move || {
                let _ = app.forensics_pg.since_edit.set_text("");
                let _ = app.forensics_pg.until_edit.set_text("");
                let _ = app.forensics_pg.inc_folder_edit.set_text("");
                let _ = app.forensics_pg.exc_folder_edit.set_text("*Trash*;*Spam*");
                app.forensics_pg.chk_deleted.set_check(false);
                {
                    let mut st = app.state.lock().unwrap();
                    st.filter_since.clear();
                    st.filter_until.clear();
                    st.filter_include_folder.clear();
                    st.filter_exclude_folder = "*Trash*;*Spam*".to_string();
                    st.filter_include_deleted = false;
                }
                app.refresh_message_list();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.forensics_pg.btn_browse_csv.on().bn_clicked(move || {
                if let Ok(Some(p)) = pick_save_file(
                    app.wnd.hwnd(),
                    "Select Forensic Manifest CSV",
                    "forensic_manifest.csv",
                    "csv",
                    &[("CSV (*.csv)", "*.csv"), ("All Files", "*.*")],
                ) {
                    let s = p.to_string_lossy().to_string();
                    let _ = app.forensics_pg.manifest_edit.set_text(&s);
                    app.state.lock().unwrap().manifest_csv_path = s;
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.forensics_pg.btn_stamp.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let txt = {
                    let mut st = app.state.lock().unwrap();
                    st.execute_forensics_stamping();
                    to_crlf(st.forensics_result.as_deref().unwrap_or(""))
                };
                let _ = app.forensics_pg.result_edit.set_text(&txt);
                app.refresh_message_list();
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.forensics_pg.btn_dry_run.on().bn_clicked(move || {
                let txt = to_crlf(&app.state.lock().unwrap().compute_dry_run_summary());
                let _ = app.forensics_pg.result_edit.set_text(&txt);
                Ok(())
            });
        }

        // PST Validator page
        {
            let app = self.clone();
            self.val_pg.btn_browse.on().bn_clicked(move || {
                if let Ok(Some(p)) = pick_open_file(
                    app.wnd.hwnd(),
                    "Select PST or OST File",
                    &[("Outlook Storage Files", "*.pst;*.ost"), ("All Files", "*.*")],
                    None,
                ) {
                    let s = p.to_string_lossy().to_string();
                    let _ = app.val_pg.path_edit.set_text(&s);
                    app.state.lock().unwrap().validate_pst_path = s;
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.val_pg.btn_validate.on().bn_clicked(move || {
                app.sync_controls_to_state();
                let mut st = app.state.lock().unwrap();
                st.execute_validate_pst();
                let rep = to_crlf(st.validation_report.as_deref().unwrap_or(""));
                drop(st);
                let _ = app.val_pg.report_edit.set_text(&rep);
                Ok(())
            });
        }

        // Outlook & Office Diagnostics page (Tab 7 - guap-gui-win32 + guap-export)
        {
            let app = self.clone();
            self.outlook_pg.btn_browse.on().bn_clicked(move || {
                if let Ok(Some(p)) = pick_open_file(
                    app.wnd.hwnd(),
                    "Select Outlook/Office Artifact (.nk2, .dat, .pst, .ost, .msg, .oab, .vcf, hive)",
                    &[
                        (
                            "Outlook & Office Files",
                            "*.nk2;*.dat;*.pst;*.ost;*.msg;*.eml;*.mbox;*.oab;*.vcf",
                        ),
                        ("All Files", "*.*"),
                    ],
                    None,
                ) {
                    let s = p.to_string_lossy().to_string();
                    let _ = app.outlook_pg.source_edit.set_text(&s);
                    app.state.lock().unwrap().outlook_source_path = s;
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.outlook_pg.btn_scan.on().bn_clicked(move || {
                let tool_idx = app.outlook_pg.tool_combo.items().selected_index().unwrap_or(0) as usize;
                let src = app.outlook_pg.source_edit.text().unwrap_or_default();
                let res = app.state.lock().unwrap().load_outlook_tool(tool_idx, &src);
                match res {
                    Ok(_) => app.refresh_outlook_grid(true),
                    Err(err) => {
                        let _ = app.outlook_pg.detail_edit.set_text(&to_crlf(&format!("Error: {err}")));
                    }
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.outlook_pg.filter_edit.on().en_change(move || {
                let txt = app.outlook_pg.filter_edit.text().unwrap_or_default();
                app.state.lock().unwrap().outlook_grid.set_filter(txt);
                app.refresh_outlook_grid(false);
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.outlook_pg.list.on().lvn_item_changed(move |p| {
                if p.uNewState.has(co::LVIS::SELECTED) && p.iItem >= 0 {
                    let detail = {
                        let mut st = app.state.lock().unwrap();
                        st.outlook_grid.selected_row = Some(p.iItem as usize);
                        st.outlook_grid.format_inspector_crlf()
                    };
                    let _ = app.outlook_pg.detail_edit.set_text(&detail);
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.outlook_pg.list.on().lvn_column_click(move |p| {
                if p.iSubItem >= 0 {
                    app.state.lock().unwrap().outlook_grid.toggle_sort(p.iSubItem as usize);
                    app.refresh_outlook_grid(false);
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.outlook_pg.btn_export.on().bn_clicked(move || {
                if let Ok(Some(p)) = pick_save_file(
                    app.wnd.hwnd(),
                    "Export Outlook/Office Grid",
                    "outlook_export.csv",
                    "csv",
                    &[
                        ("CSV Spreadsheet (*.csv)", "*.csv"),
                        ("JSON (*.json)", "*.json"),
                        ("Markdown (*.md)", "*.md"),
                    ],
                ) {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("csv");
                    let fmt = guap_export::ExportFormat::parse(ext).unwrap_or(guap_export::ExportFormat::Csv);
                    let res = app.state.lock().unwrap().export_outlook_grid(&p, fmt);
                    match res {
                        Ok(msg) => {
                            let _ = app
                                .wnd
                                .hwnd()
                                .MessageBox(&msg, "Export Complete", co::MB::ICONINFORMATION);
                        }
                        Err(err) => {
                            let _ = app.wnd.hwnd().MessageBox(&err, "Export Failed", co::MB::ICONERROR);
                        }
                    }
                }
                Ok(())
            });
        }
        {
            let app = self.clone();
            self.outlook_pg.btn_mutate.on().bn_clicked(move || {
                let (tool_idx, src, selected_prog_id, selected_enabled) = {
                    let st = app.state.lock().unwrap();
                    let prog_id = if st.outlook_tool_idx == 4 {
                        st.outlook_grid
                            .selected_row
                            .map(|r| st.outlook_grid.cell_text(r, 2))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let enabled = if st.outlook_tool_idx == 4 {
                        st.outlook_grid
                            .selected_row
                            .map(|r| st.outlook_grid.cell_text(r, 8).eq_ignore_ascii_case("true"))
                            .unwrap_or(true)
                    } else {
                        true
                    };
                    (st.outlook_tool_idx, st.outlook_source_path.clone(), prog_id, enabled)
                };

                if tool_idx == 0 && !src.is_empty() {
                    let Ok(co::DLGID::YES) = app.wnd.hwnd().MessageBox(
                        &format!("Deduplicate and atomically save '{src}' (creating .bak)?"),
                        "Confirm Mutating NK2 Edit",
                        co::MB::YESNO | co::MB::ICONWARNING,
                    ) else {
                        return Ok(());
                    };
                    let p = Path::new(&src);
                    if let Ok(mut arc) = mail2rust_outlook::Nk2Archive::from_file(p) {
                        let removed = arc.dedup();
                        match arc.save_atomic(p, true) {
                            Ok(()) => {
                                let _ = app.state.lock().unwrap().load_outlook_tool(0, &src);
                                app.refresh_outlook_grid(true);
                                let _ = app.wnd.hwnd().MessageBox(
                                    &format!("Deduplicated {removed} duplicate NK2 entry(ies) and saved atomically."),
                                    "NK2 Saved",
                                    co::MB::ICONINFORMATION,
                                );
                            }
                            Err(e) => {
                                let _ = app
                                    .wnd
                                    .hwnd()
                                    .MessageBox(&e.to_string(), "NK2 Error", co::MB::ICONERROR);
                            }
                        }
                    }
                } else if tool_idx == 4 && !selected_prog_id.is_empty() {
                    let target_enable = !selected_enabled;
                    let action_lbl = if target_enable { "ENABLE" } else { "DISABLE" };
                    let Ok(co::DLGID::YES) = app.wnd.hwnd().MessageBox(
                        &format!("{action_lbl} Office add-in '{selected_prog_id}' in live Windows Registry?"),
                        "Confirm Mutating Registry Toggle",
                        co::MB::YESNO | co::MB::ICONWARNING,
                    ) else {
                        return Ok(());
                    };
                    let backend = mail2rust_outlook::LiveWindowsRegistry;
                    match mail2rust_outlook::toggle_office_addin(
                        &backend,
                        &selected_prog_id,
                        None,
                        target_enable,
                        false,
                    ) {
                        Ok((reg_path, old_v, new_v)) => {
                            let _ = app.state.lock().unwrap().load_outlook_tool(4, &src);
                            app.refresh_outlook_grid(true);
                            let _ = app.wnd.hwnd().MessageBox(
                                &format!("Updated {reg_path}: LoadBehavior {old_v} -> {new_v}"),
                                "Add-in Updated",
                                co::MB::ICONINFORMATION,
                            );
                        }
                        Err(e) => {
                            let _ =
                                app.wnd
                                    .hwnd()
                                    .MessageBox(&e.to_string(), "Registry Toggle Failed", co::MB::ICONERROR);
                        }
                    }
                } else {
                    let _ = app.wnd.hwnd().MessageBox(
                        "Load an NK2 file (Tool 0) to deduplicate or select an Office Add-in (Tool 4) to toggle.",
                        "Outlook & Office",
                        co::MB::ICONINFORMATION,
                    );
                }
                Ok(())
            });
        }

        // Menu command dispatcher
        let wire_menu = |id: u16, handler: Box<dyn Fn(&Win32App)>| {
            let app = self.clone();
            self.wnd.on().wm_command_acc_menu(id, move || {
                handler(&app);
                Ok(())
            });
        };

        wire_menu(IDM_OPEN_FILE, Box::new(|app| app.open_mailbox_file_dialog()));
        wire_menu(IDM_OPEN_FOLDER, Box::new(|app| app.open_mailbox_folder_dialog()));
        wire_menu(
            IDM_LOAD_SAMPLE,
            Box::new(|app| {
                app.state.lock().unwrap().load_sample_data();
                app.refresh_all_views();
            }),
        );
        wire_menu(
            IDM_EXPORT_ALL,
            Box::new(|app| {
                app.export_pg.scope_combo.items().select(Some(0));
                app.state.lock().unwrap().export_scope = ExportScope::All;
                app.select_tab_index(1);
            }),
        );
        wire_menu(
            IDM_EXPORT_FILTERED,
            Box::new(|app| {
                app.export_pg.scope_combo.items().select(Some(1));
                app.state.lock().unwrap().export_scope = ExportScope::Filtered;
                app.select_tab_index(1);
            }),
        );
        wire_menu(
            IDM_EXPORT_SELECTED,
            Box::new(|app| {
                app.export_pg.scope_combo.items().select(Some(2));
                app.state.lock().unwrap().export_scope = ExportScope::Selected;
                app.select_tab_index(1);
            }),
        );
        wire_menu(
            IDM_EXIT,
            Box::new(|app| {
                let _ = app.wnd.hwnd().DestroyWindow();
            }),
        );
        wire_menu(
            IDM_TOGGLE_ROW_SEL,
            Box::new(|app| {
                let mut st = app.state.lock().unwrap();
                if let Some(id) = st.selected_message_id.clone()
                    && !st.selected_message_ids.remove(&id)
                {
                    st.selected_message_ids.insert(id);
                }
                drop(st);
                app.refresh_message_list();
            }),
        );
        wire_menu(
            IDM_SELECT_ALL,
            Box::new(|app| {
                app.state.lock().unwrap().select_all_filtered();
                app.refresh_message_list();
            }),
        );
        wire_menu(
            IDM_INVERT_SEL,
            Box::new(|app| {
                app.state.lock().unwrap().invert_selection_filtered();
                app.refresh_message_list();
            }),
        );
        wire_menu(
            IDM_CLEAR_SEL,
            Box::new(|app| {
                app.state.lock().unwrap().clear_selection();
                app.refresh_message_list();
            }),
        );
        for (cmd_id, ptab) in [
            (IDM_PREV_BODY, PreviewTab::Body),
            (IDM_PREV_HEADERS, PreviewTab::Headers),
            (IDM_PREV_MAPI, PreviewTab::Mapi),
            (IDM_PREV_FORENSICS, PreviewTab::Forensics),
        ] {
            wire_menu(
                cmd_id,
                Box::new(move |app| {
                    app.state.lock().unwrap().preview_tab = ptab;
                    app.refresh_preview();
                }),
            );
        }
        wire_menu(
            IDM_REFRESH,
            Box::new(|app| {
                app.state.lock().unwrap().rebuild_folders_from_messages();
                app.refresh_all_views();
            }),
        );
        wire_menu(
            IDM_CLEAR_SEARCH,
            Box::new(|app| {
                let _ = app.wb_pg.search_edit.set_text("");
                app.state.lock().unwrap().search_query.clear();
                app.refresh_message_list();
            }),
        );
        for tab_idx in 0..7u32 {
            wire_menu(
                IDM_TAB_BASE + tab_idx as u16,
                Box::new(move |app| app.select_tab_index(tab_idx)),
            );
        }
        wire_menu(
            IDM_CLEAR_DATA,
            Box::new(|app| {
                app.state.lock().unwrap().clear_data();
                app.refresh_all_views();
            }),
        );
        wire_menu(
            IDM_ABOUT,
            Box::new(|app| {
                let _ = app.wnd.hwnd().MessageBox(
                    &format!(
                        "Mail2Rust Studio v{} (Native Win32 / WinSafe)\n\n\
                         - 100% Pure Rust offline email & [MS-PST] engine\n\
                         - Zero Outlook / COM / Electron dependencies\n\
                         - Cross-platform alternative: mail2rust gui --iced\n\
                         - Apache License, Version 2.0",
                        env!("CARGO_PKG_VERSION")
                    ),
                    "About Mail2Rust Studio",
                    co::MB::ICONINFORMATION,
                );
            }),
        );
    }
}
