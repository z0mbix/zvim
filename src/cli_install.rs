//! Install a launcher beside the user's other commands, without external tools.
use anyhow::{Context, Result, bail};
use std::{
    io::Write,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
};

const HEADER: &[u8] = b"#!/bin/sh\n# Zvim-managed CLI launcher\n";

pub fn default_bin_directory() -> PathBuf {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".local/bin"))
        .unwrap_or_else(|| PathBuf::from("/usr/local/bin"))
}

pub fn install_current(directory: &Path) -> Result<PathBuf> {
    let executable =
        std::env::current_exe().context("Cannot locate the running Zvim application")?;
    if executable
        .components()
        .any(|part| part.as_os_str() == "AppTranslocation")
    {
        bail!("Move Zvim.app to its permanent location, reopen it, then install the command.");
    }
    install(&executable, directory)
}

fn install(executable: &Path, directory: &Path) -> Result<PathBuf> {
    if !directory.is_absolute() {
        bail!("Choose an absolute bin directory.");
    }
    let executable = executable
        .canonicalize()
        .context("Cannot locate the Zvim executable")?;
    if !executable.is_file() {
        bail!("The Zvim executable is not a file.");
    }
    std::fs::create_dir_all(directory).with_context(|| {
        format!(
            "Cannot create {}. Choose a directory you can write to.",
            directory.display()
        )
    })?;
    let directory = directory.canonicalize()?;
    let destination = directory.join("zvim");
    let existing = match std::fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            if !metadata.is_file() || !std::fs::read(&destination)?.starts_with(HEADER) {
                bail!(
                    "An unrelated command already exists at {}. Choose another directory or move it yourself.",
                    destination.display()
                );
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    let mut file = tempfile::NamedTempFile::new_in(&directory).with_context(|| {
        format!(
            "Cannot write to {}. Choose a directory you can write to.",
            directory.display()
        )
    })?;
    file.write_all(HEADER)?;
    file.write_all(b"exec '")?;
    // Quote bytes rather than UTF-8 so paths with spaces, apostrophes, shell
    // metacharacters or non-UTF-8 names retain their exact meaning.
    for byte in executable.as_os_str().as_bytes() {
        if *byte == b'\'' {
            file.write_all(b"'\\''")?;
        } else {
            file.write_all(&[*byte])?;
        }
    }
    file.write_all(b"' --zvim-launch \"$@\"\n")?;
    file.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    file.as_file().sync_all()?;
    if existing {
        file.persist(&destination).map_err(|error| error.error)?;
    } else {
        file.persist_noclobber(&destination)
            .map_err(|error| error.error)?;
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launcher_preserves_arguments_cwd_and_quoted_app_path() {
        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("Zvim's $(test) app");
        std::fs::write(&binary, b"#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\"\n").unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let bin = root.path().join("new bin");
        let launcher = install(&binary, &bin).unwrap();
        let output = std::process::Command::new(&launcher)
            .current_dir(root.path())
            .args(["a b", "'quoted'", "$(literal)", "--wait"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!(
                "{}\n--zvim-launch\na b\n'quoted'\n$(literal)\n--wait\n",
                root.path().canonicalize().unwrap().display()
            )
        );
        assert_eq!(install(&binary, &bin).unwrap(), launcher);
    }
    #[test]
    fn refuses_unrelated_files_directories_and_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("app");
        std::fs::write(&binary, b"binary").unwrap();
        let bin = root.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let dest = bin.join("zvim");
        std::fs::write(&dest, b"unrelated").unwrap();
        assert!(install(&binary, &bin).is_err());
        assert_eq!(std::fs::read(&dest).unwrap(), b"unrelated");
        std::fs::remove_file(&dest).unwrap();
        std::os::unix::fs::symlink(&binary, &dest).unwrap();
        assert!(install(&binary, &bin).is_err());
        std::fs::remove_file(&dest).unwrap();
        std::fs::create_dir(&dest).unwrap();
        assert!(install(&binary, &bin).is_err());
        assert!(install(&binary, Path::new("relative/bin")).is_err());
    }
}
