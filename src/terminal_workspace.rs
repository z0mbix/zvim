//! Versioned project terminal layouts. Shell processes and scrollback stay in memory only.
use crate::terminal_layout::SavedLayout;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, fs::DirBuilderExt},
    path::{Path, PathBuf},
    sync::{OnceLock, mpsc},
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedTerminal {
    pub cwd: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWorkspace {
    pub layout: SavedLayout<SavedTerminal>,
    pub visible: bool,
    pub fraction: f32,
    pub maximized: bool,
}

#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    project: PathBuf,
    workspace: SavedWorkspace,
}

impl SavedWorkspace {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.fraction.is_finite() && self.fraction > 0. && self.fraction < 1.,
            "Invalid saved dock height"
        );
        self.layout.validate()
    }
}

fn filename(directory: &Path, project: &Path) -> PathBuf {
    let digest = Sha256::digest(project.as_os_str().as_bytes());
    directory.join(format!("{digest:x}.json"))
}

pub fn load_from(directory: &Path, project: &Path) -> Result<Option<SavedWorkspace>> {
    let file = match std::fs::File::open(filename(directory, project)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(256 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 256 * 1024,
        "Saved terminal layout is too large"
    );
    let document: Document =
        serde_json::from_slice(&bytes).context("Cannot read saved terminal layout")?;
    ensure!(
        document.version == 1 && document.project == project,
        "Saved terminal layout has an incompatible version or project"
    );
    document.workspace.validate()?;
    Ok(Some(document.workspace))
}

fn write(directory: &Path, project: PathBuf, workspace: SavedWorkspace) -> Result<()> {
    workspace.validate()?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)?;
    let destination = filename(directory, &project);
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(
        &mut file,
        &Document {
            version: 1,
            project,
            workspace,
        },
    )?;
    file.flush()?;
    file.persist(destination)?;
    Ok(())
}

enum Request {
    Save(PathBuf, PathBuf, SavedWorkspace),
    Flush(mpsc::Sender<()>),
}

/// One writer serialises atomic per-project replacements across application windows.
struct Writer(mpsc::Sender<Request>);

impl Writer {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("zvim-terminal-layouts".into())
            .spawn(move || {
                let mut written = std::collections::HashMap::new();
                while let Ok(request) = receiver.recv() {
                    match request {
                        Request::Save(directory, project, workspace) => {
                            let key = (directory.clone(), project.clone());
                            if written.get(&key) == Some(&workspace) {
                                continue;
                            }
                            match write(&directory, project, workspace.clone()) {
                                Ok(()) => {
                                    written.insert(key, workspace);
                                }
                                Err(error) => eprintln!("Cannot save terminal layout: {error:#}"),
                            }
                        }
                        Request::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("start terminal layout writer");
        Self(sender)
    }

    fn flush(&self) {
        let (sender, receiver) = mpsc::channel();
        if self.0.send(Request::Flush(sender)).is_ok() {
            let _ = receiver.recv();
        }
    }
}

static WRITER: OnceLock<Writer> = OnceLock::new();

pub fn directory() -> PathBuf {
    crate::settings::data_dir().join("terminal-layouts")
}

pub fn save_in(directory: PathBuf, project: PathBuf, workspace: SavedWorkspace) {
    let _ = WRITER
        .get_or_init(Writer::new)
        .0
        .send(Request::Save(directory, project, workspace));
}

pub fn flush() {
    if let Some(writer) = WRITER.get() {
        writer.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_layout::{Axis, TerminalLayout};

    fn workspace(cwd: &Path) -> SavedWorkspace {
        let mut layout = TerminalLayout::default();
        layout.push(SavedTerminal {
            cwd: cwd.to_owned(),
        });
        let pane = layout.focused_pane().unwrap();
        layout.split(
            pane,
            Axis::Horizontal,
            SavedTerminal {
                cwd: cwd.join("subdir"),
            },
        );
        SavedWorkspace {
            layout: layout.snapshot(Clone::clone),
            visible: true,
            fraction: 0.6,
            maximized: false,
        }
    }

    #[test]
    fn projects_are_isolated_and_last_save_is_flushed_atomically() {
        use std::os::unix::fs::PermissionsExt;
        let temporary = tempfile::tempdir().unwrap();
        let a = Path::new("/projects/a");
        let b = Path::new("/projects/b");
        let writer = Writer::new();
        let first = workspace(a);
        let mut last = first.clone();
        last.fraction = 0.3;
        writer
            .0
            .send(Request::Save(temporary.path().to_owned(), a.into(), first))
            .unwrap();
        writer
            .0
            .send(Request::Save(
                temporary.path().to_owned(),
                b.into(),
                workspace(b),
            ))
            .unwrap();
        writer
            .0
            .send(Request::Save(
                temporary.path().to_owned(),
                a.into(),
                last.clone(),
            ))
            .unwrap();
        writer.flush();
        assert_eq!(load_from(temporary.path(), a).unwrap(), Some(last));
        assert_eq!(load_from(temporary.path(), b).unwrap(), Some(workspace(b)));
        assert_eq!(
            std::fs::metadata(filename(temporary.path(), a))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn absent_corrupt_mismatched_and_future_layouts_do_not_restore() {
        let temporary = tempfile::tempdir().unwrap();
        let project = Path::new("/project");
        assert!(load_from(temporary.path(), project).unwrap().is_none());
        let path = filename(temporary.path(), project);
        std::fs::write(&path, "broken JSON").unwrap();
        assert!(load_from(temporary.path(), project).is_err());
        for (version, root) in [(2, project), (1, Path::new("/other"))] {
            std::fs::write(
                &path,
                serde_json::to_vec(&Document {
                    version,
                    project: root.into(),
                    workspace: workspace(project),
                })
                .unwrap(),
            )
            .unwrap();
            assert!(load_from(temporary.path(), project).is_err());
        }
    }
}
