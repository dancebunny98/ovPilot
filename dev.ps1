$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $project
$localCargo = Join-Path $project '.toolchain\cargo\bin\cargo.exe'
$cargo = if (Test-Path -LiteralPath $localCargo) { $localCargo } else { 'cargo' }
if ($cargo -eq $localCargo) {
    $env:CARGO_HOME = Join-Path $project '.toolchain\cargo'
    $env:RUSTUP_HOME = Join-Path $project '.toolchain\rustup'
    $tools = Join-Path $project '.toolchain\rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained'
    $env:PATH = "$tools;$env:CARGO_HOME\bin;$env:PATH"
}
$logDirectory = Join-Path $project 'logs'
New-Item -ItemType Directory -Path $logDirectory -Force | Out-Null
$log = Join-Path $logDirectory 'dev.log'
$fingerprint = ''
$client = $null
Write-Host 'Watching Rust sources. Close this console to stop watching.'
while ($true) {
    $files = @(Get-ChildItem -LiteralPath (Join-Path $project 'src') -Recurse -File -Filter '*.rs')
    $files += Get-Item -LiteralPath (Join-Path $project 'Cargo.toml')
    $current = ($files | Sort-Object FullName | ForEach-Object { "$($_.FullName):$($_.LastWriteTimeUtc.Ticks)" }) -join '|'
    if ($current -ne $fingerprint) {
        $fingerprint = $current
        if ($client -and -not $client.HasExited) {
            Stop-Process -Id $client.Id -ErrorAction SilentlyContinue
            $client.WaitForExit()
        }
        Write-Host 'Building ovPilot...'
        & $cargo build 2>&1 | Tee-Object -FilePath $log
        if ($LASTEXITCODE -eq 0) {
            $client = Start-Process -FilePath (Join-Path $project 'target\debug\ovPilot.exe') -ArgumentList 'run' -PassThru
            Write-Host "ovPilot is visible (PID $($client.Id))."
        } else {
            Write-Host "Build failed. See $log."
            if (Test-Path -LiteralPath (Join-Path $project 'target\debug\ovPilot.exe')) {
                $client = Start-Process -FilePath (Join-Path $project 'target\debug\ovPilot.exe') -ArgumentList 'run' -PassThru
            }
        }
    }
    Start-Sleep -Milliseconds 700
}
