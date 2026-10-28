# Privacy and credentials

ovPilot saves non-secret preferences in the user's local settings directory.
The VATSIM password is stored separately in the operating system credential
vault after successful verification. The password is not included in the
project source, installer, build artifacts, or GitHub Actions configuration.

The `Forget saved password` control removes the saved credential. Do not add
local `settings.json`, logs, runtime directories, `.env` files, tokens, or
credential exports to commits or bug reports. Build outputs, local toolchains,
runtime state, original vPilot binaries, and research files are excluded from
the repository by `.gitignore` and the publication file list.

The bundled aircraft type list is derived from a public MIT-licensed dataset;
its attribution and license are in `data/`.
