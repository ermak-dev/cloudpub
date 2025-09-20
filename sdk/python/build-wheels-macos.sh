#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

OUTPUT_DIR="./dist"

# Set environment variables
export NEXT_PUBLIC_VERSION="$(cat ../../VERSION)"
export NEXT_PUBLIC_DOMAIN=cloudpub.ru
export NEXT_PUBLIC_PORT=443
export NEXT_PUBLIC_ONPREM=false
export NEXT_PUBLIC_SITE_NAME=CloudPub

# Check if running on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo -e "${RED}Error: This script must be run on macOS${NC}"
    echo "For Linux/Windows builds, use ./build-wheels.sh"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

echo -e "${GREEN}Building CloudPub Python SDK wheels natively on macOS${NC}"
echo "Output directory: $OUTPUT_DIR"
echo ""

# Check for required tools
echo -e "${BLUE}Checking required tools...${NC}"

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Rust not found. Installing via rustup...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# Check for Python
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}Python 3 not found. Please install Python 3 first.${NC}"
    echo "You can install it via Homebrew: brew install python3"
    exit 1
fi

# Check for maturin
if ! python3 -m pip show maturin &> /dev/null; then
    echo -e "${YELLOW}Installing maturin...${NC}"
    python3 -m pip install --user maturin
fi

# Add Rust targets
echo -e "${BLUE}Adding Rust targets...${NC}"
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# Build for both architectures
echo ""
echo -e "${YELLOW}Building macOS aarch64 (Apple Silicon) wheel...${NC}"
maturin build --release \
    --target aarch64-apple-darwin \
    --out "$OUTPUT_DIR"

echo ""
echo -e "${YELLOW}Building macOS x86_64 (Intel) wheel...${NC}"
maturin build --release \
    --target x86_64-apple-darwin \
    --out "$OUTPUT_DIR"

echo -e "${GREEN}Done!${NC}"
