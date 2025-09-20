#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
PYTHON_VERSIONS="3.11 3.12 3.13 3.14"
TARGET_PLATFORMS="linux_x86_64 linux_aarch64 macos_x86_64 macos_aarch64 windows_x86_64"
OUTPUT_DIR="./dist"

# Function to print usage
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -p, --python VERSIONS     Python versions to build for (default: '3.11 3.12 3.13 3.14')"
    echo "  -a, --arch PLATFORMS      Target platforms (default: all platforms)"
    echo "  -o, --output DIR          Output directory for wheels (default: ./dist)"
    echo "  -c, --clean               Clean build cache before building"
    echo "  -h, --help                Show this help message"
    echo ""
    echo "Target platforms:"
    echo "  linux_x86_64    - Linux x86_64 (Intel/AMD)"
    echo "  linux_aarch64   - Linux ARM64"
    echo "  macos_x86_64    - macOS Intel"
    echo "  macos_aarch64   - macOS Apple Silicon"
    echo "  windows_x86_64  - Windows x86_64"
    echo ""
    echo "Examples:"
    echo "  $0                                    # Build for all platforms and Python versions"
    echo "  $0 -p '3.11 3.12'                     # Build only for Python 3.11 and 3.12"
    echo "  $0 -a 'linux_x86_64 macos_aarch64'   # Build only for specific platforms"
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -p|--python)
            PYTHON_VERSIONS="$2"
            shift 2
            ;;
        -a|--arch)
            TARGET_PLATFORMS="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -c|--clean)
            CLEAN_BUILD=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            usage
            exit 1
            ;;
    esac
done

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Clean previous builds if requested
if [ "$CLEAN_BUILD" = true ]; then
    echo -e "${YELLOW}Cleaning previous builds...${NC}"
    rm -rf "$OUTPUT_DIR"/*.whl
    docker rmi cloudpub-python-sdk-builder 2>/dev/null || true
fi

echo -e "${GREEN}Building CloudPub Python SDK wheels${NC}"
echo "Python versions: $PYTHON_VERSIONS"
echo "Target platforms: $TARGET_PLATFORMS"
echo "Output directory: $OUTPUT_DIR"
echo ""

# Export environment variables for docker-compose
export PYTHON_VERSIONS
export TARGET_PLATFORMS

# Navigate to project root (assuming script is in sdk/python/)
cd "$(dirname "$0")/../.."

# Run the Docker build
echo -e "${YELLOW}Building wheels using Docker...${NC}"
docker build \
    -t cloudpub-python-sdk-builder \
    -f sdk/python/Dockerfile \
    --build-arg PYTHON_VERSIONS="${PYTHON_VERSIONS}" \
    --build-arg TARGET_PLATFORMS="${TARGET_PLATFORMS}" \
    .

# Create a container and copy the wheels out
echo -e "${YELLOW}Extracting wheels from container...${NC}"
CONTAINER_ID=$(docker create cloudpub-python-sdk-builder)
docker cp "${CONTAINER_ID}:/output/." "${OUTPUT_DIR}/"
docker rm "${CONTAINER_ID}"

# Check if any wheels were built
if ls "${OUTPUT_DIR}"/*.whl 1> /dev/null 2>&1; then
    echo -e "${GREEN}Build completed successfully!${NC}"
    echo "Wheels created:"
    ls -lh "${OUTPUT_DIR}"/*.whl
else
    echo -e "${RED}No wheels were created. Build may have failed.${NC}"
    exit 1
fi

# Optional: Test import of wheels
echo ""
echo -e "${YELLOW}To test the wheels, you can install them with:${NC}"
echo "pip install ${OUTPUT_DIR}/cloudpub_python_sdk-*.whl"