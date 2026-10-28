# vPilot 3.12.1 installer analysis

## Source and method

- Official download: `https://vpilot.rosscarlson.dev/Assets/Files/Installers/vPilot-Setup-3.12.1.exe`
- Installer SHA-256: `528A51BF0E71ADA11103314D18A9FC0321AFAC94E2706F2FB13E0FBF5BB4BEAA`; size: 13,984,016 bytes.
- Windows Authenticode status: valid; signer subject: Metacraft.
- 7-Zip identifies the embedded archive as NSIS-2. Its extraction produced 39 files (23,755,626 uncompressed bytes). The 34 application files are retained in `vendor/vpilot-3.12.1/`.
- REA `inspect-managed-artifact` and `inspect-managed-members` were used on `vPilot.exe`. The full temporary evidence bundle was removed during project cleanup. This was static analysis; the target was not executed.

## Installed application payload

All paths below are relative to the NSIS installation directory. This table lists the 34 application files present in the installer archive, excluding temporary installer UI files and the NSIS uninstaller blob.

| Path | Role visible from name/metadata |
| --- | --- |
| `vPilot.exe` | Main application |
| `vPilot.exe.config` | .NET runtime configuration |
| `vPilotConfig.xml` | Supplied application configuration |
| `ModelMatchingData.bin` | Model matching data |
| `Public.key` | Supplied key data |
| `GeoVR.Client.dll` | GeoVR client dependency |
| `GeoVR.Connection.dll` | GeoVR connection dependency |
| `GeoVR.Shared.dll` | GeoVR shared dependency |
| `RossCarlson.Vatsim.Vpilot.Plugins.dll` | vPilot plugins assembly |
| `RossCarlson.Vatsim.Vpilot.Plugins.xml` | Plugins XML documentation |
| `Microsoft.FlightSimulator.SimConnect.dll` | Simulator interoperability dependency |
| `abacus.dll` | Bundled dependency |
| `bbv.Common.EventBroker.dll` | Bundled dependency |
| `BouncyCastle.Crypto.dll` | Bundled dependency |
| `Castle.Core.dll` | Bundled dependency |
| `CoreCompat.Portable.Licensing.dll` | Bundled dependency |
| `INIFileParser.dll` | Bundled dependency |
| `log4net.dll` | Bundled dependency |
| `Newtonsoft.Json.dll` | Bundled dependency |
| `Ninject.dll` | Bundled dependency |
| `Ninject.Extensions.Factory.dll` | Bundled dependency |
| `protobuf-net.dll` | Bundled dependency |
| `Serilog.dll` | Bundled dependency |
| `Serilog.Sinks.File.dll` | Bundled dependency |
| `SharpDX.DirectInput.dll` | Bundled dependency |
| `SharpDX.dll` | Bundled dependency |
| `System.Net.Http.Formatting.dll` | Bundled dependency |
| `Sounds/Broadcast.wav` | Notification audio |
| `Sounds/DirectRadioMessage.wav` | Notification audio |
| `Sounds/Error.wav` | Notification audio |
| `Sounds/NewMessage.wav` | Notification audio |
| `Sounds/PrivateMessage.wav` | Notification audio |
| `Sounds/RadioMessage.wav` | Notification audio |
| `Sounds/SelCal.wav` | Notification audio |

Installer-only archive entries: `$PLUGINSDIR/InstallOptions.dll`, `$PLUGINSDIR/ioSpecial.ini`, `$PLUGINSDIR/modern-header.bmp`, `$PLUGINSDIR/modern-wizard.bmp`, and `Uninstall.exe.nsis`. The last item is an NSIS extraction artifact and is **not** a directly runnable uninstaller.

The default `$INSTDIR`, registry writes, shortcuts, and generated uninstaller path cannot be established from 7-Zip extraction alone. They have not been verified by running the installer. The Rust deployment tool uses its own explicit local destination and does not claim to reproduce those system changes.

## REA observations

- `vPilot.exe`: SHA-256 `C743F204929309DB6F49E8B543DB2D661AD09C32A2A7BC71FE419C685D244BFF`, version `3.12.1.0`, PE x86, .NET Framework 4.7.2, CIL.
- Metadata reports 591 types, 4,474 methods, 2,705 fields, 2,040 user strings and 28 assembly references. `DotfuscatorAttribute` and short type names indicate obfuscation of parts of the assembly.
- Named types and literals show simulator integration (FSX, Prepar3D v2-v6, MSFS, MSFS 2024), VATSIM network connection, Host/Remote modes, radio/audio/PTT, model matching and `.vmr` rule sets, notifications, updates, and settings.
- A literal says the application exits if no supported flight simulator is detected. The provided `vPilot.exe.config` requests .NET Framework 4.7.2.
- Static metadata does not establish live network behavior, UI appearance, file writes after startup, or compatibility of a Rust rewrite.

## Reproduction boundary

This analysis describes the original installer as a reference. The current native Rust application does not load or launch those files. It implements a desktop window, aircraft file inventory, release check, X-Plane telemetry probe, VATSIM JWT credential verification, and system-vault password storage. A successful credential check does not establish a VATSIM network session. FSD login/session handling, voice, simulator traffic, and complete model matching remain unimplemented and require separate design and live verification.
