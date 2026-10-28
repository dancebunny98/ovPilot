@echo off
setlocal EnableExtensions
cd /d "%~dp0" || exit /b 1
where pwsh >nul 2>&1
if not errorlevel 1 (
    pwsh -NoProfile -File "%~dp0dev.ps1"
) else (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0dev.ps1"
)
exit /b %ERRORLEVEL%
