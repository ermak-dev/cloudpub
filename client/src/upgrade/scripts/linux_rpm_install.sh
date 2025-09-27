#!/bin/bash
set -e

PACKAGE_PATH="{PACKAGE_PATH}"
CURRENT_EXE="{CURRENT_EXE}"
CURRENT_ARGS="{CURRENT_ARGS}"

echo "Installing RPM package..."

# Check if running as root, if not use pkexec
if [ "$EUID" -ne 0 ]; then
    echo "Root privileges required for package installation"
    pkexec rpm -Uvh "$PACKAGE_PATH"
else
    rpm -Uvh "$PACKAGE_PATH"
fi

if [ $? -ne 0 ]; then
    echo "Failed to install RPM package"
    exit 1
fi

echo "RPM package installed successfully"

# Wait a moment for the process to exit
sleep 3

# Restart the application
exec "$CURRENT_EXE" $CURRENT_ARGS
