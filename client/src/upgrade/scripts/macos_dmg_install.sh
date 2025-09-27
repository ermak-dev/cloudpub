#!/bin/bash
set -e

PACKAGE_PATH="{PACKAGE_PATH}"
CURRENT_EXE="{CURRENT_EXE}"
CURRENT_ARGS="{CURRENT_ARGS}"

echo "Installing DMG package..."

# Mount the DMG
MOUNT_OUTPUT=$(hdiutil attach -nobrowse -quiet "$PACKAGE_PATH")
if [ $? -ne 0 ]; then
    echo "Failed to mount DMG package"
    exit 1
fi

# Extract mount point
MOUNT_POINT=$(echo "$MOUNT_OUTPUT" | grep "/Volumes/" | awk '{print $NF}')
if [ -z "$MOUNT_POINT" ]; then
    echo "Could not find mount point"
    exit 1
fi

# Find .app bundle
APP_PATH=$(find "$MOUNT_POINT" -name "*.app" -type d | head -n 1)
if [ -z "$APP_PATH" ]; then
    hdiutil detach "$MOUNT_POINT" -quiet
    echo "No .app bundle found in DMG"
    exit 1
fi

APP_NAME=$(basename "$APP_PATH")
DESTINATION="/Applications/$APP_NAME"

# Remove existing app if it exists
if [ -d "$DESTINATION" ]; then
    rm -rf "$DESTINATION"
fi

# Copy app to Applications
cp -R "$APP_PATH" "/Applications/"
if [ $? -ne 0 ]; then
    hdiutil detach "$MOUNT_POINT" -quiet
    echo "Failed to copy app to Applications"
    exit 1
fi

# Unmount DMG
hdiutil detach "$MOUNT_POINT" -quiet

echo "DMG package installed successfully"

# Launch new version
APP_NAME_NO_EXT=$(basename "$APP_NAME" .app)
open -a "$APP_NAME_NO_EXT"
echo "Launched new application version"

# Clean up script
rm -f "$0"
