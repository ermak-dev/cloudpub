#!/bin/bash
set -e

# Wait for parent process to exit
sleep 2

# Log file for debugging
LOG_FILE="{LOG_FILE}"

# Function to log messages (only to file, not console)
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" >> "$LOG_FILE"
}

log "==============================================="
log "CloudPub Service Update Script"
log "==============================================="
log "Script: $0"
log "PID: $$"
log "User: $(whoami)"
log "Working directory: $(pwd)"

NEW_EXE="{NEW_EXE}"
CURRENT_EXE="{CURRENT_EXE}"
SERVICE_ARGS="{SERVICE_ARGS}"

log "New executable: $NEW_EXE"
log "Current executable: $CURRENT_EXE"
log "Service arguments: $SERVICE_ARGS"
log "Starting service update process..."

"$CURRENT_EXE" service stop >> "$LOG_FILE" 2>&1 || {
    log "Warning: Service stop command failed (may already be stopped)"
}

log "Waiting for service to fully stop..."
sleep 1

# Ensure the process is stopped
log "Ensuring all service processes are stopped..."
pkill -f "clo.*--run-as-service" >> "$LOG_FILE" 2>&1 || {
    log "No running service processes found"
}
sleep 1

log "Replacing executable..."
cp -f "$NEW_EXE" "$CURRENT_EXE" >> "$LOG_FILE" 2>&1
if [ $? -eq 0 ]; then
    log "Executable replaced successfully"
else
    log "ERROR: Failed to replace executable"
    exit 1
fi

log "Setting executable permissions..."
chmod 755 "$CURRENT_EXE" >> "$LOG_FILE" 2>&1

log "Starting CloudPub service with new executable and config..."

log "Starting service with command: $CURRENT_EXE service start"
"$CURRENT_EXE" service start >> "$LOG_FILE" 2>&1

if [ $? -eq 0 ]; then
    log "Service started successfully"
    log "Update completed successfully"
    log "Script kept at: $0"
    log "Log file: $LOG_FILE"
    exit 0
else
    log "ERROR: Failed to start service, you may need to start it manually"
    log "Script kept at: $0"
    log "Log file: $LOG_FILE"
    exit 1
fi
