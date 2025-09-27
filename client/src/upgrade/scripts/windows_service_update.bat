@echo off
setlocal enabledelayedexpansion

set "LOG_FILE={LOG_FILE}"

echo [%date% %time%] =============================================== >> "%LOG_FILE%"
echo [%date% %time%] CloudPub Service Update Script >> "%LOG_FILE%"
echo [%date% %time%] =============================================== >> "%LOG_FILE%"
echo [%date% %time%] Script: %~f0 >> "%LOG_FILE%"
echo [%date% %time%] User: %USERNAME% >> "%LOG_FILE%"
echo [%date% %time%] Working directory: %CD% >> "%LOG_FILE%"
echo [%date% %time%] Starting service update process... >> "%LOG_FILE%"

echo CloudPub Service Update Script
echo Log file: %LOG_FILE%

echo [%date% %time%] Attempting to stop CloudPub service... >> "%LOG_FILE%"
"{CURRENT_EXE}" service stop >> "%LOG_FILE%" 2>&1
if errorlevel 1 (
    echo [%date% %time%] Direct service stop failed, trying sc command... >> "%LOG_FILE%"
    sc stop cloudpub >> "%LOG_FILE%" 2>&1
    if errorlevel 1 (
        echo [%date% %time%] Warning: Failed to stop service using sc command >> "%LOG_FILE%"
    )
)

echo [%date% %time%] Waiting for service to fully stop... >> "%LOG_FILE%"
timeout /t 10 /nobreak >nul

echo [%date% %time%] Checking if old executable is still in use... >> "%LOG_FILE%"
:retry_move
echo [%date% %time%] Attempting to replace executable... >> "%LOG_FILE%"
move /y "{NEW_EXE}" "{CURRENT_EXE}" >> "%LOG_FILE%" 2>&1
if errorlevel 1 (
    echo [%date% %time%] Failed to replace executable, waiting and retrying... >> "%LOG_FILE%"
    timeout /t 2 /nobreak >nul
    taskkill /F /IM clo.exe >> "%LOG_FILE%" 2>&1
    goto retry_move
)

echo [%date% %time%] Executable successfully replaced >> "%LOG_FILE%"
echo [%date% %time%] Starting CloudPub service... >> "%LOG_FILE%"

"{CURRENT_EXE}" service start >> "%LOG_FILE%" 2>&1
if errorlevel 1 (
    echo [%date% %time%] Direct service start failed, trying sc command... >> "%LOG_FILE%"
    sc start cloudpub >> "%LOG_FILE%" 2>&1
    if errorlevel 1 (
        echo [%date% %time%] Failed to start service using sc command >> "%LOG_FILE%"
        echo [%date% %time%] Please start the service manually >> "%LOG_FILE%"
        echo [%date% %time%] Script kept at: %~f0 >> "%LOG_FILE%"
        echo [%date% %time%] Log file: %LOG_FILE% >> "%LOG_FILE%"
        echo ERROR: Failed to start service. Check log file: %LOG_FILE%
        pause
        exit /b 1
    )
)

echo [%date% %time%] Service update completed successfully >> "%LOG_FILE%"
echo [%date% %time%] Script kept at: %~f0 >> "%LOG_FILE%"
echo [%date% %time%] Log file: %LOG_FILE% >> "%LOG_FILE%"
echo Service update completed. Check log: %LOG_FILE%
timeout /t 2 /nobreak >nul
rem Script is kept for debugging - not deleted
