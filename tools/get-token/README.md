# Cursor Token Retrieval Tool

This tool is used to retrieve access tokens from the Cursor editor's local database.

## System Requirements

- Rust programming environment
- Cargo package manager

## Build Instructions

### Windows

1. Install Rust
   ```powershell
   winget install Rustlang.Rust
   # Or visit https://rustup.rs/ to download the installer
   ```

2. Clone the project and build
   ```powershell
   git clone <repository-url>
   cd get-token
   cargo build --release
   ```

3. After building, the executable is located at `target/release/get-token.exe`

### macOS

1. Install Rust
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Clone the project and build
   ```bash
   git clone <repository-url>
   cd get-token
   cargo build --release
   ```

3. After building, the executable is located at `target/release/get-token`

## Usage

Simply run the compiled executable:

- Windows: `.\target\release\get-token.exe`
- macOS: `./target/release/get-token`

The program will automatically find and display the Cursor editor's access token.

## Notes

- Ensure the Cursor editor is installed and has been logged in at least once
- Windows database path: `%USERPROFILE%\AppData\Roaming\Cursor\User\globalStorage\state.vscdb`
- macOS database path: `~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`