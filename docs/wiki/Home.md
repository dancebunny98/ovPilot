# ovPilot documentation

ovPilot is a standalone Rust desktop pilot client under development. It does
not load or execute vPilot binaries. Version 1.0.0 has a working desktop UI,
local settings, aircraft inventory, and diagnostic network and simulator code.
The final Connect action is disabled while a complete FSD session and simulator
integration are being implemented.

## Guides

- [Build and run](Build-and-Run.md)
- [Features and limits](Features-and-Limits.md)
- [Privacy and credentials](Privacy-and-Credentials.md)
- [Releases and versions](Releases.md)
- [Network, chat, and voice status](Network-and-Voice.md)
- [X-Plane 11/12 status](../XPLANE.md)
- [Reference installer analysis](../ANALYSIS.md)

The Rust API reference is generated from English doc comments with
`cargo doc --no-deps --open`.
