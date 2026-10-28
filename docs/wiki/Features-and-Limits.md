# Features and limits

## Available in version 1.0.0

- Native Rust desktop window with Messages, Notes, Controllers In Range,
  Settings, and About.
- Persistent local preferences, window layout, and notes.
- Aircraft inventory for MSFS/Prepar3D `aircraft.cfg` and X-Plane 11/12 `.acf`.
- Connect Type Code suggestions from 2,688 bundled offline designators.
- Custom `.vmr` model matching rule parsing.
- VATSIM credential verification and password storage in the operating
  system credential vault.
- Audio device selection, microphone test and monitor, and PTT key capture.
- GitHub Releases version check and read-only X-Plane UDP telemetry probe.

## Incomplete

Credential verification does not connect the client to VATSIM. FSD session
login, live ATC messaging, voice service, aircraft traffic injection, radio
integration, flight plans, and automatic model matching are incomplete. The
Connect button stays disabled until the required network and simulator path
works end to end. X-Plane 11 and 12 have not been verified with a live simulator.

The type catalog is a source of suggestions, not a complete validation list:
you can type a code that is not in the bundled data.
