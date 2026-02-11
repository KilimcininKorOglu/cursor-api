#!/bin/bash
set -euo pipefail

# Color output functions
info() { echo -e "\033[1;34m[INFO]\033[0m $*"; }
warn() { echo -e "\033[1;33m[WARN]\033[0m $*"; }
error() { echo -e "\033[1;31m[ERROR]\033[0m $*" >&2; exit 1; }

# Check required tools
check_requirements() {
    local missing_tools=()

    # Basic tool check
    for tool in cargo protoc npm node; do
        if ! command -v "$tool" &>/dev/null; then
            missing_tools+=("$tool")
        fi
    done

    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        error "Missing required tools: ${missing_tools[*]}"
    fi
}

# Parse arguments
USE_STATIC=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --static) USE_STATIC=true ;;
        --help) show_help; exit 0 ;;
        *) error "Unknown argument: $1" ;;
    esac
    shift
done

# Help message
show_help() {
    cat << EOF
Usage: $(basename "$0") [options]

Options:
  --static        Use static linking (default is dynamic linking)
  --help          Show this help message

Without arguments, only compiles for current platform
EOF
}

# Parallel build function
build_target() {
    local target=$1
    local extension=""
    local rustflags="${2:-}"

    info "Building $target..."

    # Determine file extension
    [[ $target == *"windows"* ]] && extension=".exe"

    # Build
    if [[ $target != "$CURRENT_TARGET" ]]; then
        env RUSTFLAGS="$rustflags" cargo build --target "$target" --release
    else
        env RUSTFLAGS="$rustflags" cargo build --release
    fi

    # Move build artifacts to release directory
    local binary_name="cursor-api"
    [[ $USE_STATIC == true ]] && binary_name+="-static"

    local binary_path
    if [[ $target == "$CURRENT_TARGET" ]]; then
        binary_path="target/release/cursor-api$extension"
    else
        binary_path="target/$target/release/cursor-api$extension"
    fi

    if [[ -f "$binary_path" ]]; then
        cp "$binary_path" "release/${binary_name}-$target$extension"
        info "Completed building $target"
    else
        warn "Build artifact not found: $target"
        warn "Search path: $binary_path"
        warn "Current directory contents:"
        ls -R target/
        return 1
    fi
}

# Get CPU architecture and operating system
ARCH=$(uname -m | sed 's/^aarch64\|arm64$/aarch64/;s/^x86_64\|x86-64\|x64\|amd64$/x86_64/')
OS=$(uname -s)

# Determine current system's target platform
get_target() {
    local arch=$1
    local os=$2
    case "$os" in
        "Darwin") echo "${arch}-apple-darwin" ;;
        "Linux") 
            if [[ $USE_STATIC == true ]]; then
                echo "${arch}-unknown-linux-musl"
            else
                echo "${arch}-unknown-linux-gnu"
            fi
            ;;
        "MINGW"*|"MSYS"*|"CYGWIN"*|"Windows_NT") echo "${arch}-pc-windows-msvc" ;;
        "FreeBSD") echo "${arch}-unknown-freebsd" ;;
        *) error "Unsupported system: $os" ;;
    esac
}

# Set current target platform
CURRENT_TARGET=$(get_target "$ARCH" "$OS")

# Check if target platform was successfully determined
[ -z "$CURRENT_TARGET" ] && error "Unable to determine current system's target platform"

# Get all targets for the system
get_targets() {
    case "$1" in
        "linux")
            # Linux only builds current architecture
            echo "$CURRENT_TARGET"
            ;;
        "freebsd")
            # FreeBSD only builds current architecture
            echo "$CURRENT_TARGET"
            ;;
        "windows")
            # Windows only builds current architecture
            echo "$CURRENT_TARGET"
            ;;
        "macos")
            # macOS builds all macOS targets
            echo "x86_64-apple-darwin aarch64-apple-darwin"
            ;;
        *) error "Unsupported system group: $1" ;;
    esac
}

# Check dependencies
check_requirements

# Determine targets to build
case "$OS" in
    Darwin) 
        TARGETS=($(get_targets "macos"))
        ;;
    Linux)
        TARGETS=($(get_targets "linux"))
        ;;
    FreeBSD)
        TARGETS=($(get_targets "freebsd"))
        ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
        TARGETS=($(get_targets "windows"))
        ;;
    *) error "Unsupported system: $OS" ;;
esac

# Create release directory
mkdir -p release

# Set static linking flags
RUSTFLAGS="-C link-arg=-s"
[[ $USE_STATIC == true ]] && RUSTFLAGS="-C target-feature=+crt-static -C link-arg=-s"

# Build all targets in parallel
info "Starting build..."
for target in "${TARGETS[@]}"; do
    build_target "$target" "$RUSTFLAGS" &
done

# Wait for all builds to complete
wait

# Create universal binary for macOS platform
if [[ "$OS" == "Darwin" ]] && [[ ${#TARGETS[@]} -gt 1 ]]; then
    binary_suffix=""
    [[ $USE_STATIC == true ]] && binary_suffix="-static"

    if [[ -f "release/cursor-api${binary_suffix}-x86_64-apple-darwin" ]] && \
       [[ -f "release/cursor-api${binary_suffix}-aarch64-apple-darwin" ]]; then
        info "Creating macOS universal binary..."
        lipo -create \
            "release/cursor-api${binary_suffix}-x86_64-apple-darwin" \
            "release/cursor-api${binary_suffix}-aarch64-apple-darwin" \
            -output "release/cursor-api${binary_suffix}-universal-apple-darwin"
    fi
fi

info "Build completed!"