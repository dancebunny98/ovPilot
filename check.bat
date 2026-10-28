@echo off
setlocal EnableExtensions
cd /d "%~dp0" || exit /b 1
set "CARGO_CMD=cargo"
if exist ".toolchain\cargo\bin\cargo.exe" (
    set "CARGO_HOME=%CD%\.toolchain\cargo"
    set "RUSTUP_HOME=%CD%\.toolchain\rustup"
    set "PATH=%CD%\.toolchain\rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained;%CD%\.toolchain\cargo\bin;%PATH%"
    set "CARGO_CMD=%CD%\.toolchain\cargo\bin\cargo.exe"
)
if not exist "logs" mkdir "logs" || exit /b 1
set "LOG=%CD%\logs\check.log"
>"%LOG%" echo [check] Started %DATE% %TIME%

if "%CARGO_CMD%"=="cargo" (
    where cargo >nul 2>&1
    if errorlevel 1 (
        >>"%LOG%" echo ERROR: Cargo is not on PATH. Install Rust and restart the IDE terminal.
        goto failed
    )
)

>>"%LOG%" echo [check] Rust formatting
call "%CARGO_CMD%" fmt --all -- --check >>"%LOG%" 2>&1
if errorlevel 1 goto failed

>>"%LOG%" echo [check] Clippy
call "%CARGO_CMD%" clippy --all-targets -- -D warnings >>"%LOG%" 2>&1
if errorlevel 1 goto failed

>>"%LOG%" echo [check] Tests
call "%CARGO_CMD%" test >>"%LOG%" 2>&1
if errorlevel 1 goto failed

>>"%LOG%" echo [check] API documentation
set "RUSTDOCFLAGS=-D warnings"
call "%CARGO_CMD%" doc --no-deps >>"%LOG%" 2>&1
if errorlevel 1 goto failed

>>"%LOG%" echo [check] Completed successfully
type "%LOG%"
exit /b 0

:failed
set "CODE=%ERRORLEVEL%"
if "%CODE%"=="0" set "CODE=1"
>>"%LOG%" echo [check] Failed with exit code %CODE%
type "%LOG%"
exit /b %CODE%
