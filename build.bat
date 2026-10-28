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
set "LOG=%CD%\logs\build.log"
>"%LOG%" echo [build] Started %DATE% %TIME%

call "%~dp0check.bat" >>"%LOG%" 2>&1
if errorlevel 1 goto failed

>>"%LOG%" echo [build] Release binary
call "%CARGO_CMD%" build --release >>"%LOG%" 2>&1
if errorlevel 1 goto failed
if not exist "target\release\ovPilot.exe" (
    >>"%LOG%" echo ERROR: Release binary was not produced.
    goto failed
)

if not exist "release" mkdir "release" || goto failed
copy /y "target\release\ovPilot.exe" "release\ovPilot.exe" >>"%LOG%" 2>&1
if errorlevel 1 goto failed
copy /y "data\aircraft_types-LICENSE" "release\AircraftTypes-LICENSE.txt" >>"%LOG%" 2>&1
if errorlevel 1 goto failed

if not exist "docs\api" mkdir "docs\api" || goto failed
xcopy "target\doc\*" "docs\api\" /E /I /Y /Q >>"%LOG%" 2>&1
if errorlevel 1 goto failed

>>"%LOG%" echo [build] Completed successfully
type "%LOG%"
exit /b 0

:failed
set "CODE=%ERRORLEVEL%"
if "%CODE%"=="0" set "CODE=1"
>>"%LOG%" echo [build] Failed with exit code %CODE%
type "%LOG%"
exit /b %CODE%
