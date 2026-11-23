#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod app;
mod message;
mod state;
mod views;

use app::Mail2RustApp;
use mail2rust_core::init_logging;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchArgs {
    pub path: Option<PathBuf>,
    pub sample: bool,
    pub pick: bool,
}

pub fn parse_launch_args_from<I>(args: I) -> LaunchArgs
where
    I: IntoIterator<Item = String>,
{
    let mut out = LaunchArgs::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--sample" => out.sample = true,
            "--pick" => out.pick = true,
            "--path" => {
                if let Some(p) = iter.next() {
                    out.path = Some(PathBuf::from(p));
                }
            }
            "--help" | "-h" => {
                println!(
                    "mail2rust-iced [PATH] [--path DIR_OR_FILE] [--sample] [--pick]\n\
                     Launch the cross-platform Iced 0.14 desktop studio for mail2rust (Linux, macOS, Windows)."
                );
                std::process::exit(0);
            }
            "--version" | "-V" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            other if !other.starts_with('-') && out.path.is_none() => {
                out.path = Some(PathBuf::from(other));
            }
            _ => {}
        }
    }
    out
}

const APP_ICON_RGBA: &[u8] = include_bytes!("../assets/icon_32x32.rgba");

fn main() -> iced::Result {
    let _ = init_logging("warn", None);
    let launch = parse_launch_args_from(std::env::args().skip(1));
    let icon = iced::window::icon::from_rgba(APP_ICON_RGBA.to_vec(), 32, 32).ok();

    iced::application(
        move || Mail2RustApp::new(launch.clone()),
        Mail2RustApp::update,
        Mail2RustApp::view,
    )
    .title(Mail2RustApp::title)
    .theme(Mail2RustApp::theme)
    .window(iced::window::Settings {
        size: iced::Size::new(1360.0, 820.0),
        min_size: Some(iced::Size::new(900.0, 540.0)),
        position: iced::window::Position::Centered,
        icon,
        ..Default::default()
    })
    .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_launch_args_with_path_and_sample() {
        let parsed = parse_launch_args_from(["/tmp/mailbox.mbox", "--sample"].into_iter().map(str::to_string));
        assert!(parsed.sample);
        assert_eq!(parsed.path, Some(PathBuf::from("/tmp/mailbox.mbox")));
    }
}
