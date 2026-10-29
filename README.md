# ovPilot 1.0.0

ovPilot is a native Rust desktop client in development. The default `run` command opens our own window; it does not execute or load the original vPilot client. The UI follows the compact window layout observed during analysis. The source includes English Rust API documentation.

## Implemented

- Native desktop window with resizable Controllers In Range, Messages, Notes, Settings, and About below Miscellaneous. Notes and local preferences persist in the user settings directory.
- Background aircraft scan of `aircraft.cfg` and X-Plane 11/12 `.acf` files. MSFS and MSFS 2024 package roots are read from `UserCfg.opt`. `ovPilot scan [paths...]` is the CLI equivalent.
- Connect Type Code suggestions from a bundled offline catalog of 2,688 distinct designators. The field remains editable for codes absent from the catalog.
- Custom `.vmr` XML rule parsing and ordered matching against installed model names. Rule-set paths can be managed in Settings.
- VATSIM FSD JWT request with an in-memory password and a local HTTP test. Network Settings can check credentials, but this does not establish a full FSD session.
- Bounded FSD text, ping/pong, and ATC update decoding; nearby ATC grouping and frequency formatting have local tests.
- GitHub Releases update check (`ovPilot update check` or Settings > Updates).
- Read-only X-Plane 11/12 UDP telemetry probe (`ovPilot xplane probe [host:port]`). This has a local protocol test, but has not been tested against a live simulator.

## Not yet implemented

Live VATSIM authentication has not been tested with a user account. FSD session login, server connection, voice, ATC message exchange, traffic injection, radios, flight plans, and automatic model matching are not complete. The Connect dialog opens, but its final Connect action and related controls are disabled. The `.acf` inventory and UDP probe do not establish full X-Plane compatibility. No native Linux or macOS build has been verified.

## Build

Open this directory in VS Code or IntelliJ IDEA. On Windows, run `dev.bat` for a visible window that rebuilds and restarts after source edits. Run `check.bat`, `build.bat`, and `run.bat` for verification and release use. The scripts log failures in `logs/`. A Rust toolchain and a complete Windows linker toolchain are required. `cargo build --release` produces `target/release/ovPilot.exe`; `build.bat` copies it to `release/`.

`installer/ovPilot.nsi` is the source for our Windows installer. With NSIS installed, `build-installer.bat` builds `installer/ovPilot-Setup-1.0.0.exe`. GitHub Actions builds Windows, Linux, and macOS binaries and the Windows installer; a successful CI run is still needed to verify those builds. Original vPilot binaries and research tools are not included in this repository and are not dependencies of the native app.

## Documentation

- [Project guide](docs/wiki/Home.md)
- [Build and run](docs/wiki/Build-and-Run.md)
- [Features and limits](docs/wiki/Features-and-Limits.md)
- [Privacy and credentials](docs/wiki/Privacy-and-Credentials.md)
- [Releases and versions](docs/wiki/Releases.md)
- [Network, chat, and voice status](docs/wiki/Network-and-Voice.md)
- [X-Plane 11/12 status](docs/XPLANE.md)
- [Reference installer analysis](docs/ANALYSIS.md)

Rust API documentation is generated with `cargo doc --no-deps --open`. The project repository is https://github.com/dancebunny98/ovPilot.
