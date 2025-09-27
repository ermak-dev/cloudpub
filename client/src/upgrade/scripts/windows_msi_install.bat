@echo off
timeout /t 3 /nobreak >nul
echo Installing MSI package...
echo Requesting administrator privileges...
powershell -Command "Start-Process msiexec -ArgumentList '/i', '{MSI_PATH}', '/quiet', '/norestart' -Verb RunAs -Wait"
if errorlevel 1 (
    echo Failed to install MSI package
    exit /b 1
)
echo Installation completed successfully
start "" "{CURRENT_EXE}"
