#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod win32_app;
#[cfg(windows)]
mod win32_dialogs;
#[cfg(any(windows, test))]
#[cfg_attr(not(windows), allow(dead_code))]
mod win32_state;

use mail2rust_core::init_logging;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct LaunchArgs {
    pub path: Option<PathBuf>,
    pub sample: bool,
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
            "--path" => {
                if let Some(p) = iter.next() {
                    out.path = Some(PathBuf::from(p));
                }
            }
            "--help" | "-h" => {
                println!(
                    "mail2rust-win32 [PATH] [--path DIR_OR_FILE] [--sample]\n\
                     Launch the native Win32/WinSafe Mail2Rust Studio."
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

fn main() {
    let _ = init_logging("warn", None);
    let launch_args = parse_launch_args_from(std::env::args().skip(1));

    #[cfg(windows)]
    {
        if let Err(err) = win32_app::run(launch_args) {
            eprintln!("mail2rust-win32 fatal error: {err}");
            std::process::exit(1);
        }
    }

    #[cfg(not(windows))]
    {
        let _ = launch_args;
        eprintln!("mail2rust-win32 is a Windows-native Win32 binary. Run `mail2rust-iced` on Linux/macOS.");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_launch_args_with_path_and_sample() {
        let parsed = parse_launch_args_from(["--sample", r"C:\Mail\Archive.pst"].into_iter().map(str::to_string));
        assert!(parsed.sample);
        assert_eq!(parsed.path, Some(PathBuf::from(r"C:\Mail\Archive.pst")));
    }
}
