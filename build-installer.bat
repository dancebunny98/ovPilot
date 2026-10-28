@echo off
setlocal EnableExtensions
cd /d "%~dp0" || exit /b 1
if not exist "logs" mkdir "logs" || exit /b 1
set "LOG=%CD%\logs\installer.log"
>"%LOG%" echo [installer] Started %DATE% %TIME%
call "%~dp0build.bat" >>"%LOG%" 2>&1
if errorlevel 1 goto failed
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
"%NSIS_CMD%" /DSOURCE_EXE="..\release\ovPilot.exe" ovPilot.nsi >>"%LOG%" 2>&1
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
