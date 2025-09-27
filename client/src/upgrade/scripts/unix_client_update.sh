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
log "CloudPub Client Update Script"
log "==============================================="
log "Script: $0"
log "PID: $$"
log "User: $(whoami)"
log "Working directory: $(pwd)"

NEW_EXE="{NEW_EXE}"
CURRENT_EXE="{CURRENT_EXE}"
ARGS="{ARGS}"

log "New executable: $NEW_EXE"
log "Current executable: $CURRENT_EXE"
log "Arguments: $ARGS"
log "Starting client update process..."

log "Waiting for current process to exit..."
sleep 2

log "Replacing executable..."
# Use cp -f to force overwrite even if file is busy
cp -f "$NEW_EXE" "$CURRENT_EXE" >> "$LOG_FILE" 2>&1
if [ $? -eq 0 ]; then
    log "Executable replaced successfully"
else
    log "ERROR: Failed to replace executable"
    exit 1
fi

log "Setting executable permissions..."
chmod 755 "$CURRENT_EXE" >> "$LOG_FILE" 2>&1

log "Starting new version with args: $ARGS"
log "Script kept at: $0"
log "Log file: $LOG_FILE"
exec "$CURRENT_EXE" $ARGS

# This line shouldn't be reached if exec succeeds
log "ERROR: exec failed - this should not happen"
