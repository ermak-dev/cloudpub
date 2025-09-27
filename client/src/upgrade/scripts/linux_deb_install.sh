#!/bin/bash
set -e

PACKAGE_PATH="{PACKAGE_PATH}"
CURRENT_EXE="{CURRENT_EXE}"
CURRENT_ARGS="{CURRENT_ARGS}"

echo "Installing DEB package..."

# Check if running as root, if not use pkexec
if [ "$EUID" -ne 0 ]; then
    echo "Root privileges required for package installation"
    pkexec dpkg -i "$PACKAGE_PATH"
else
    dpkg -i "$PACKAGE_PATH"
fi

if [ $? -ne 0 ]; then
    echo "Failed to install DEB package"
    exit 1
fi

echo "DEB package installed successfully"

# Wait a moment for the process to exit
sleep 3

# Restart the application
exec "$CURRENT_EXE" $CURRENT_ARGS
