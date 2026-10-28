param([Parameter(Mandatory)][string]$Cargo)

$ErrorActionPreference = 'Stop'
$metadata = & $Cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) {
    throw 'Cargo metadata failed.'
}
$package = $metadata.packages | Where-Object name -eq 'ovpilot' | Select-Object -First 1
if (-not $package -or $package.version -notmatch '^\d+\.\d+\.\d+$') {
    throw 'Cargo.toml must declare a stable ovpilot major.minor.patch version.'
}
$package.version
