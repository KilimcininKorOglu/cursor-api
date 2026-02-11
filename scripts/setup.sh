#!/bin/bash

# Exit on error
set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO] $1${NC}"
}

error() {
    echo -e "${RED}[ERROR] $1${NC}"
    exit 1
}

# Check if running as root (FreeBSD and Linux)
if [ "$(uname)" != "Darwin" ] && [ "$EUID" -ne 0 ]; then
    error "Please run this script with root privileges (sudo ./setup.sh)"
fi

# Detect package manager
if command -v brew &> /dev/null; then
    PKG_MANAGER="brew"
    info "Detected macOS/Homebrew system"
elif command -v pkg &> /dev/null; then
    PKG_MANAGER="pkg"
    info "Detected FreeBSD system"
elif command -v apt-get &> /dev/null; then
    PKG_MANAGER="apt-get"
    info "Detected Debian/Ubuntu system"
elif command -v dnf &> /dev/null; then
    PKG_MANAGER="dnf"
    info "Detected Fedora/RHEL system"
elif command -v yum &> /dev/null; then
    PKG_MANAGER="yum"
    info "Detected CentOS system"
else
    error "No supported package manager detected"
fi

# Update package manager cache
info "Updating package manager cache..."
case $PKG_MANAGER in
    "brew")
        brew update
        ;;
    "pkg")
        pkg update
        ;;
    *)
        $PKG_MANAGER update -y
        ;;
esac

# Install basic build tools
info "Installing basic build tools..."
case $PKG_MANAGER in
    "brew")
        brew install \
            protobuf \
            pkg-config \
            openssl \
            curl \
            git \
            node
        ;;
    "pkg")
        pkg install -y \
            gmake \
            protobuf \
            pkgconf \
            openssl \
            curl \
            git \
            node
        ;;
    "apt-get")
        $PKG_MANAGER install -y --no-install-recommends \
            build-essential \
            protobuf-compiler \
            pkg-config \
            libssl-dev \
            ca-certificates \
            curl \
            tzdata \
            git
        ;;
    *)
        $PKG_MANAGER install -y \
            gcc \
            gcc-c++ \
            make \
            protobuf-compiler \
            pkg-config \
            openssl-devel \
            ca-certificates \
            curl \
            tzdata \
            git
        ;;
esac

# Install Node.js and npm (if not already installed via package manager)
if ! command -v node &> /dev/null && [ "$PKG_MANAGER" != "brew" ] && [ "$PKG_MANAGER" != "pkg" ]; then
    info "Installing Node.js and npm..."
    if [ "$PKG_MANAGER" = "apt-get" ]; then
        curl -fsSL https://deb.nodesource.com/setup_lts.x | bash -
        $PKG_MANAGER install -y nodejs
    else
        curl -fsSL https://rpm.nodesource.com/setup_lts.x | bash -
        $PKG_MANAGER install -y nodejs
    fi
fi

# Install Rust (if not installed)
if ! command -v rustc &> /dev/null; then
    info "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
fi

# Add target platforms
info "Adding Rust target platforms..."
case "$(uname)" in
    "FreeBSD")
        rustup target add x86_64-unknown-freebsd
        ;;
    "Darwin")
        rustup target add x86_64-apple-darwin aarch64-apple-darwin
        ;;
    *)
        rustup target add x86_64-unknown-linux-gnu
        ;;
esac

# Clean package manager cache
case $PKG_MANAGER in
    "apt-get")
        rm -rf /var/lib/apt/lists/*
        ;;
    "pkg")
        pkg clean -y
        ;;
esac

# Set timezone (except macOS)
if [ "$(uname)" != "Darwin" ]; then
    info "Setting timezone to Asia/Shanghai..."
    ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime
fi

echo -e "${GREEN}Installation complete!${NC}"