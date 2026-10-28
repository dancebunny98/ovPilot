# X-Plane 11/12 compatibility status

## Current result

The native Rust client can inventory X-Plane 11/12 `.acf` files and query a running simulator through read-only UDP RREF. It does not yet provide VATSIM traffic injection, radio/voice, or a complete simulator backend.

```powershell
.\release\ovPilot.exe xplane probe 127.0.0.1:49000
```

The probe requests `sim/version/xplane_internal_version`, latitude, longitude, and elevation. Its shared datarefs and packet parser are tested with a local UDP responder. No X-Plane installation was found on the analysis host, so a live X-Plane 11 or 12 session was not verified.

## Evidence

- REA 6.8.0 `inspect-managed-members` inspected the official vPilot 3.12.1 executable (SHA-256 `C743F204929309DB6F49E8B543DB2D661AD09C32A2A7BC71FE419C685D244BFF`). Its 2,040 decoded user strings include the explicit error: "Please ensure you have FSX, Prepar3D, or MSFS installed correctly." No decoded user string named X-Plane. This supports, but alone does not prove, the absence of an X-Plane backend.
- REA inspected `RossCarlson.Vatsim.Vpilot.Plugins.dll`: its public `IPlugin` and `IBroker` methods expose network events and commands, but no simulator backend registration or SimConnect replacement method was observed.
- REA classified the supplied xPilot 4.0.0-beta.9 Windows installer as native x86_64 PE without .NET CIL metadata, and the Linux AppImage as ELF. The supplied macOS archive contains an `.app` installer. These are installers for another client, not plugins for the unchanged vPilot UI.
- The supplied `pilotclient-main.zip` contains swift pilot client source. Its README explicitly lists X-Plane 11/12 on Windows, Linux and macOS. Its X-Plane adapter calls a separate `xswiftbus` plugin over DBus and manages remote aircraft, cockpit, messages, model packages, and scenery. It is C++/Qt source with GPL-3.0-or-later or the swift pilot client license; no swift source was copied into this Rust project.

## Work required for full support

A functional X-Plane backend must bridge own-aircraft state, transponder/radios/PTT, remote aircraft and CSL models, simulator lifecycle, and VATSIM voice/network state. The added archives provide reference implementations and installers, but launching another client's GUI would change the interface and is not counted as ovPilot compatibility.
