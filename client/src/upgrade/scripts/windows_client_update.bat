@echo off
setlocal enabledelayedexpansion

set "LOG_FILE={LOG_FILE}"

echo [%date% %time%] =============================================== >> "%LOG_FILE%"
echo [%date% %time%] CloudPub Client Update Script >> "%LOG_FILE%"
echo [%date% %time%] =============================================== >> "%LOG_FILE%"
echo [%date% %time%] Script: %~f0 >> "%LOG_FILE%"
echo [%date% %time%] User: %USERNAME% >> "%LOG_FILE%"
echo [%date% %time%] Working directory: %CD% >> "%LOG_FILE%"
echo [%date% %time%] Starting client update process... >> "%LOG_FILE%"

echo CloudPub Client Update Script
echo Log file: %LOG_FILE%

echo [%date% %time%] Waiting for current process to exit... >> "%LOG_FILE%"
timeout /t 3 /nobreak >nul

echo [%date% %time%] Replacing executable... >> "%LOG_FILE%"
move /y "{NEW_EXE}" "{CURRENT_EXE}" >> "%LOG_FILE%" 2>&1
if errorlevel 1 (
    echo [%date% %time%] Failed to replace executable >> "%LOG_FILE%"
    echo [%date% %time%] Error: The file may be in use >> "%LOG_FILE%"
    echo [%date% %time%] Script kept at: %~f0 >> "%LOG_FILE%"
    echo [%date% %time%] Log file: %LOG_FILE% >> "%LOG_FILE%"
    echo ERROR: Failed to replace executable. Check log: %LOG_FILE%
    pause
    exit /b 1
)

echo [%date% %time%] Executable replaced successfully >> "%LOG_FILE%"
echo [%date% %time%] Starting new version... >> "%LOG_FILE%"
echo [%date% %time%] Command: "{CURRENT_EXE}" {ARGS} >> "%LOG_FILE%"
start "" "{CURRENT_EXE}" {ARGS}

echo [%date% %time%] Update completed successfully >> "%LOG_FILE%"
echo [%date% %time%] Script kept at: %~f0 >> "%LOG_FILE%"
echo [%date% %time%] Log file: %LOG_FILE% >> "%LOG_FILE%"
echo Update completed. Check log: %LOG_FILE%
timeout /t 2 /nobreak >nul
rem Script is kept for debugging - not deleted
