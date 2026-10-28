# Releases and versions

The application version comes from `Cargo.toml`. The About view and startup
log use that version. The Windows installer reads the same version when built.

On a push to `main`, the release workflow checks for a `vX.Y.Z` tag matching
the Cargo version. If the tag exists, it does nothing. If it does not exist,
the workflow builds and tests Windows, Linux, and macOS packages, then creates
a GitHub Release and tag after all three builds succeed. A new release
therefore requires a version change in `Cargo.toml`.

The release page records the full source commit SHA. Every platform archive
also includes `build-info.json` with the version, commit SHA, and platform.
The Windows installer is attached separately. The workflow uses GitHub's
short-lived `GITHUB_TOKEN`; no personal token is stored in the repository.

Builds on Linux and macOS are currently compilation checks. They do not
establish full VATSIM or simulator compatibility.
