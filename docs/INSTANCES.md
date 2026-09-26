# One application, multiple windows

CLI and direct executable launches join the running Zvim application before starting GPUI. If none exists, one launch becomes the application host. Each request creates a separate window with its own Neovim process, arguments, project directory, and caller environment. The application owns all windows and one Dock icon. Finder file-open events continue to use the app's existing open-files handler.

The installed launcher starts a detached host when needed and waits for an acknowledgement that its window was created. `--wait` waits for that window's lifetime, including any retained terminal panes, rather than the whole application. Closing another window does not release it. Closing the last app window still exits Zvim. Help and version commands do not start or contact a GUI.

A private Unix socket carries bounded, versioned launch messages. A persistent file lock elects one host even during simultaneous startup; a stale socket endpoint is replaced only after acquiring that lock. The socket lives in a short, private temporary directory to avoid macOS Unix-socket path limits. Both peers verify the other endpoint belongs to the same effective user. Arguments, paths and environment values are encoded as bytes, preserving spaces and non-UTF-8 names. Launch handling, acknowledgements and wait connections use worker threads, not blocking UI reads. Outstanding wait replies are drained when the application exits.

Neovim subprocesses receive the requesting shell's environment without changing the shared application's global environment. Embedded terminal shells use the host application's environment, and start in their window's current Neovim directory. To change the host environment or test a rebuilt/updated executable, quit Zvim first. Already-running older releases cannot be merged into the new host and should be closed once.

On macOS, Command+` cycles forward through open application windows and Command+Shift+` cycles backward, including when a terminal has focus. The Window menu exposes both actions. These shortcuts are reserved against terminal shortcut overrides.

## Validation

Unit tests cover host election, stale endpoint replacement, per-window waiting, shutdown completion, rejected opens, byte-preserving arguments/environment and oversized/incompatible messages. `python3 scripts/test-single-instance.py` opens temporary real windows with `-u NONE` and no ShaDa, confirms one shared GUI PID with independent Neovim PIDs, checks each caller's cwd/environment and filenames with spaces, and verifies independent wait completion plus direct/detached handoff. Only the test windows close themselves.

The native launch test has passed on macOS. Linux transport code is implemented but a Linux desktop runtime check remains outstanding.
