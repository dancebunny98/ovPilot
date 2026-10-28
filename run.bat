@echo off
setlocal EnableExtensions
cd /d "%~dp0" || exit /b 1
if not exist "logs" mkdir "logs" || exit /b 1
set "LOG=%CD%\logs\run.log"
>"%LOG%" echo [run] Started %DATE% %TIME%

if not exist "release\ovPilot.exe" (
    >>"%LOG%" echo ERROR: Missing release\ovPilot.exe. Run build.bat first.
    goto failed
)

"release\ovPilot.exe" run %* >>"%LOG%" 2>&1
if errorlevel 1 goto failed
>>"%LOG%" echo [run] Completed successfully
type "%LOG%"
exit /b 0

:failed
set "CODE=%ERRORLEVEL%"
if "%CODE%"=="0" set "CODE=1"
>>"%LOG%" echo [run] Failed with exit code %CODE%
type "%LOG%"
exit /b %CODE%
