//! Neovim-compatible argument forwarding with a directory operand as the project root.
use crate::session::Launch;
use anyhow::{Result, bail};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Gui,
    Detached,
    Help,
    Version,
}
pub struct Cli {
    pub launch: Launch,
    pub mode: Mode,
    pub child_args: Vec<OsString>,
    pub wait: bool,
}

pub fn parse(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Result<Cli> {
    let args: Vec<_> = args.into_iter().collect();
    let mut forwarded = Vec::new();
    let mut child_args = Vec::new();
    let mut mode = Mode::Gui;
    let mut wait = false;
    let mut from_launcher = false;
    let mut positional = false;
    let mut first_operand = true;
    let mut directory: Option<PathBuf> = None;
    let mut first_file = None;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        let text = arg.to_string_lossy();
        if !positional {
            match text.as_ref() {
                "--zvim-launch" => {
                    mode = Mode::Detached;
                    from_launcher = true;
                    i += 1;
                    continue;
                }
                "--zvim-gui" => {
                    from_launcher = true;
                    i += 1;
                    continue;
                }
                "--wait" => {
                    wait = true;
                    i += 1;
                    continue;
                }
                "--help" | "-h" | "-?" => {
                    return Ok(Cli {
                        launch: Launch::default(),
                        mode: Mode::Help,
                        child_args: vec![],
                        wait: false,
                    });
                }
                "--version" | "-v" => {
                    return Ok(Cli {
                        launch: Launch::default(),
                        mode: Mode::Version,
                        child_args: vec![],
                        wait: false,
                    });
                }
                "--" => {
                    positional = true;
                    forwarded.push(arg.clone());
                    child_args.push(arg.clone());
                    i += 1;
                    continue;
                }
                _ => {}
            }
            if text.starts_with("--headless")
                || text.starts_with("--embed")
                || text.starts_with("--remote")
                || text.starts_with("--server")
                || text.starts_with("--api-info")
            {
                bail!(
                    "{text} is not available in Zvim's embedded GUI; use nvim for headless, RPC, or remote-client modes"
                );
            }
            // Long options taking a separate value. Never interpret that value as a project directory or a Zvim flag.
            if ["--cmd", "--listen", "--startuptime"].contains(&text.as_ref()) {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("{text} requires a value"))?;
                forwarded.extend([arg.clone(), value.clone()]);
                child_args.extend([arg.clone(), value.clone()]);
                i += 2;
                continue;
            }
            if text.starts_with('-') && text.len() > 1 && !text.starts_with("--") {
                // Neovim accepts grouped switches and attached values (e.g. -RO2, -uNONE, -ccommand).
                let mut consumed = false;
                for (offset, ch) in text[1..].char_indices() {
                    let rest = &text[1 + offset + ch.len_utf8()..];
                    if matches!(ch, 'e' | 'E' | 'l') {
                        bail!(
                            "-{ch} selects a non-GUI execution mode; use nvim directly (use -S for GUI startup scripts)"
                        );
                    }
                    if matches!(ch, 'c' | 'u' | 'i' | 's' | 'w' | 'W' | 't' | 'q' | 'S') {
                        let optional = matches!(ch, 'q' | 'S');
                        let next = args.get(i + 1);
                        if rest.is_empty()
                            && (!optional
                                || next.is_some_and(|v| !v.to_string_lossy().starts_with('-')))
                        {
                            let value =
                                next.ok_or_else(|| anyhow::anyhow!("-{ch} requires a value"))?;
                            if ch == 's' && value == "-" {
                                bail!(
                                    "-s - cannot use stdin: Zvim reserves it for RPC; use -s with a script file"
                                );
                            }
                            forwarded.extend([arg.clone(), value.clone()]);
                            child_args.extend([arg.clone(), value.clone()]);
                            i += 2;
                            consumed = true;
                        } else if ch == 's' && rest == "-" {
                            bail!("-s - cannot use stdin: Zvim reserves it for RPC");
                        }
                        break;
                    }
                    if ch == 'V' {
                        break;
                    } // Remaining bytes are a level and optional log filename.
                    if matches!(ch, 'o' | 'O' | 'p') && !rest.is_empty() {
                        break;
                    } // Optional numeric count.
                }
                if consumed {
                    continue;
                }
            }
        }
        let operand = positional || (!text.starts_with('-') && !text.starts_with('+'));
        if arg == "-" && !positional {
            bail!("Reading file text from stdin is not supported by Zvim yet; open a file instead");
        }
        child_args.push(arg.clone());
        if operand && first_operand {
            first_operand = false;
            let path = cwd.join(arg);
            if path.is_dir() {
                directory = Some(path.canonicalize()?);
                i += 1;
                continue;
            }
            first_file = Some(PathBuf::from(arg));
        }
        forwarded.push(arg.clone());
        i += 1;
    }
    if wait {
        mode = Mode::Gui;
    }
    if directory.is_none() && cwd.parent().is_none() && !from_launcher {
        directory = first_file
            .filter(|p| p.is_absolute())
            .and_then(|p| p.parent().map(Path::to_owned))
            .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().to_owned()));
    }
    Ok(Cli {
        launch: Launch {
            nvim_args: forwarded,
            working_directory: Some(directory.unwrap_or_else(|| cwd.to_owned())),
            ..Default::default()
        },
        mode,
        child_args,
        wait,
    })
}

pub const HELP: &str = "Zvim — a native Neovim frontend\nUsage: zvim [options] [directory] [files...]\n\nThe first file operand, if it is an existing directory, becomes the working\ndirectory before Neovim loads your configuration. Remaining relative paths\n(including option values) are resolved there. Otherwise the shell cwd is used.\n\n  zvim .                        Open the current directory\n  zvim ~/Projects/foo           Open a project\n  zvim -O left.rs right.rs       Vertical splits\n  zvim -o2 a.txt b.txt           Horizontal splits\n  zvim -p a.txt b.txt            Tab pages\n  zvim -R file.txt               Read-only\n  zvim -d before.txt after.txt   Diff mode\n  zvim +42 src/main.rs           Start at line 42\n  zvim --wait .                 Keep the launcher attached until that window closes\n\nThe installed launcher opens a window in the running application and returns immediately.\nArguments after -- are filenames. Standard GUI-compatible Neovim arguments are\nforwarded in their original order. Headless, Ex/batch, standalone Lua, RPC,\nremote-client, and stdin-input modes are not supported. Use nvim for those.\n\nBundled Neovim options:\n";

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }
    #[test]
    fn preserves_flags_values_and_order() {
        let cwd = std::env::temp_dir();
        let input = args(&[
            "--clean",
            "-RO2",
            "-c",
            "--headless",
            "--cmd",
            "let g:test = 'space'",
            "+42",
            "a b.txt",
            "-p3",
        ]);
        let cli = parse(input.clone(), &cwd).unwrap();
        assert_eq!(cli.launch.nvim_args, input);
        assert_eq!(cli.launch.working_directory, Some(cwd));
    }
    #[test]
    fn directory_operand_changes_cwd_but_values_do_not() {
        let cwd = std::env::temp_dir().canonicalize().unwrap();
        let cli = parse(args(&["--clean", "-u", ".", ".", "-O", "one", "two"]), &cwd).unwrap();
        assert_eq!(cli.launch.working_directory, Some(cwd));
        assert_eq!(
            cli.launch.nvim_args,
            args(&["--clean", "-u", ".", "-O", "one", "two"])
        );
    }
    #[test]
    fn options_end_marker_and_literal_filenames() {
        let cli = parse(
            args(&["--", "--headless", "+42", "--wait"]),
            &std::env::temp_dir(),
        )
        .unwrap();
        assert_eq!(
            cli.launch.nvim_args,
            args(&["--", "--headless", "+42", "--wait"])
        );
    }
    #[test]
    fn launcher_flags_do_not_reach_neovim() {
        let cli = parse(
            args(&["--zvim-launch", "--wait", "-uNONE"]),
            &std::env::temp_dir(),
        )
        .unwrap();
        assert_eq!(cli.mode, Mode::Gui);
        assert!(cli.wait);
        assert_eq!(cli.child_args, args(&["-uNONE"]));
        assert_eq!(
            parse(args(&["--zvim-launch"]), &std::env::temp_dir())
                .unwrap()
                .mode,
            Mode::Detached
        );
    }
    #[test]
    fn rejects_incompatible_modes_and_missing_values() {
        for input in [
            vec!["--headless"],
            vec!["-es"],
            vec!["-ll", "script.lua"],
            vec!["--server=x"],
            vec!["--remote-tab"],
            vec!["-s", "-"],
            vec!["--cmd"],
            vec!["-u"],
        ] {
            assert!(
                parse(args(&input), &std::env::temp_dir()).is_err(),
                "{input:?}"
            );
        }
        assert!(parse(args(&["-S"]), &std::env::temp_dir()).is_ok());
        assert!(parse(args(&["-V9hello.log"]), &std::env::temp_dir()).is_ok());
    }
}
