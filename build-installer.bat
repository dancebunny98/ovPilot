@echo off
setlocal EnableExtensions
cd /d "%~dp0" || exit /b 1
if not exist "logs" mkdir "logs" || exit /b 1
set "LOG=%CD%\logs\installer.log"
>"%LOG%" echo [installer] Started %DATE% %TIME%
call "%~dp0build.bat" >>"%LOG%" 2>&1
if errorlevel 1 goto failed
set "CARGO_CMD=cargo"
if exist ".toolchain\cargo\bin\cargo.exe" (
    set "CARGO_HOME=%CD%\.toolchain\cargo"
    set "RUSTUP_HOME=%CD%\.toolchain\rustup"
    set "PATH=%CD%\.toolchain\rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained;%CD%\.toolchain\cargo\bin;%PATH%"
    set "CARGO_CMD=%CD%\.toolchain\cargo\bin\cargo.exe"
)
set "PRODUCT_VERSION="
for /f "delims=" %%V in ('powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\package-version.ps1" "%CARGO_CMD%"') do set "PRODUCT_VERSION=%%V"
if not defined PRODUCT_VERSION (
    >>"%LOG%" echo ERROR: Could not read version from Cargo metadata.
    goto failed
)
set "NSIS_CMD=makensis"
if exist ".toolchain\nsis\nsis-3.11\makensis.exe" set "NSIS_CMD=%CD%\.toolchain\nsis\nsis-3.11\makensis.exe"
if "%NSIS_CMD%"=="makensis" (
    where makensis >nul 2>&1
    if errorlevel 1 (
        >>"%LOG%" echo ERROR: NSIS makensis.exe is not on PATH.
        goto failed
    )
)
pushd installer
"%NSIS_CMD%" /DPRODUCT_VERSION="%PRODUCT_VERSION%" /DSOURCE_EXE="..\release\ovPilot.exe" ovPilot.nsi >>"%LOG%" 2>&1
if errorlevel 1 (
    popd
    goto failed
)
popd
type "%LOG%"
exit /b 0
:failed
set "CODE=%ERRORLEVEL%"
if "%CODE%"=="0" set "CODE=1"
>>"%LOG%" echo [installer] Failed with exit code %CODE%
type "%LOG%"
exit /b %CODE%
