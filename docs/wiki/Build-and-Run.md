# Build and run

## Windows

Install a current stable Rust toolchain and a working C/C++ linker. Open the
repository in VS Code or IntelliJ IDEA and run these scripts from its root:

| Command | Result |
| --- | --- |
| `check.bat` | Formatting, Clippy, tests, and Rust API docs |
| `dev.bat` | Rebuild and restart a visible development window after source edits |
| `build.bat` | Release executable in `release/ovPilot.exe` |
| `run.bat` | Run the local client |
| `build-installer.bat` | Build `installer/ovPilot-Setup-1.0.0.exe` with NSIS |

Script output and failures are recorded under the ignored `logs/` directory.
The Windows installer installs under the current user's local application
directory and provides Start Menu shortcuts and an uninstaller.

## Linux and macOS

Install stable Rust and native dependencies for eframe/egui and audio. Run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
cargo run
```

The GitHub Actions matrix compiles on Windows, Linux, and macOS. Passing a
build does not prove that all simulator, audio, or VATSIM operations work on
each platform. The NSIS installer is Windows-only.

## Diagnostics

```sh
cargo run -- scan
cargo run -- xplane probe 127.0.0.1:49000
cargo run -- update check
```

`scan` accepts optional additional aircraft directories. The X-Plane probe is
read-only and does not inject traffic into the simulator.
