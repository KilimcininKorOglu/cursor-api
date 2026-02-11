use rusqlite::{Connection, Result};
use serde_json::{Value, from_str, to_string_pretty};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

fn get_cursor_path() -> PathBuf {
    let home = if cfg!(windows) {
        env::var("USERPROFILE").unwrap_or_else(|_| env::var("HOME").unwrap())
    } else {
        env::var("HOME").unwrap()
    };

    let base_path = PathBuf::from(home);

    if cfg!(windows) {
        base_path.join("AppData\\Roaming\\Cursor")
    } else if cfg!(target_os = "macos") {
        base_path.join("Library/Application Support/Cursor")
    } else {
        base_path.join(".config/Cursor")
    }
}

fn update_sqlite_tokens(
    refresh_token: &str,
    access_token: &str,
    email: &str,
    signup_type: &str,
    membership_type: &str,
) -> Result<()> {
    let db_path = get_cursor_path().join("User/globalStorage/state.vscdb");
    let conn = Connection::open(db_path)?;

    // Get original values
    let mut stmt = conn.prepare(
        "SELECT key, value FROM ItemTable WHERE key IN (
            'cursorAuth/refreshToken',
            'cursorAuth/accessToken',
            'cursorAuth/cachedEmail',
            'cursorAuth/cachedSignUpType',
            'cursorAuth/stripeMembershipType'
        )",
    )?;

    println!("\nOriginal values:");
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (key, value) = row?;
        println!("{key}: {value}");
    }

    // Auto-create items and update values
    conn.execute(
        "INSERT OR REPLACE INTO ItemTable (key, value) VALUES ('cursorAuth/refreshToken', ?)",
        [refresh_token],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO ItemTable (key, value) VALUES ('cursorAuth/accessToken', ?)",
        [access_token],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO ItemTable (key, value) VALUES ('cursorAuth/cachedEmail', ?)",
        [email],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO ItemTable (key, value) VALUES ('cursorAuth/cachedSignUpType', ?)",
        [signup_type],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO ItemTable (key, value) VALUES ('cursorAuth/stripeMembershipType', ?)",
        [membership_type],
    )?;

    println!("\nUpdated values:");
    let mut stmt = conn.prepare(
        "SELECT key, value FROM ItemTable WHERE key IN (
            'cursorAuth/refreshToken',
            'cursorAuth/accessToken',
            'cursorAuth/cachedEmail',
            'cursorAuth/cachedSignUpType',
            'cursorAuth/stripeMembershipType'
        )",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (key, value) = row?;
        println!("{}: {}", key, value);
    }

    Ok(())
}

fn update_storage_json(machine_ids: &[String; 4]) -> io::Result<()> {
    let storage_path = get_cursor_path().join("User/globalStorage/storage.json");
    let content = fs::read_to_string(&storage_path)?;
    let mut json: Value = from_str(&content)?;

    if let Value::Object(ref mut map) = json {
        map.insert(
            "telemetry.macMachineId".to_string(),
            Value::String(machine_ids[0].clone()),
        );
        map.insert(
            "telemetry.sqmId".to_string(),
            Value::String(machine_ids[1].clone()),
        );
        map.insert(
            "telemetry.machineId".to_string(),
            Value::String(machine_ids[2].clone()),
        );
        map.insert(
            "telemetry.devDeviceId".to_string(),
            Value::String(machine_ids[3].clone()),
        );
    }

    fs::write(storage_path, to_string_pretty(&json)?)?;
    Ok(())
}

fn is_valid_jwt(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        println!("Warning: Token format incorrect, should contain 3 parts separated by '.'");
        return false;
    }

    // Check if starts with "ey"
    if !token.starts_with("ey") {
        println!("Warning: Token should start with 'ey'");
        return false;
    }

    true
}

fn is_valid_sha256(id: &str) -> bool {
    // SHA256 hash is 64 hexadecimal characters
    if id.len() != 64 {
        println!("Warning: ID length should be 64 characters");
        return false;
    }

    // Check if all are valid hexadecimal characters
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        println!("Warning: ID should only contain hexadecimal characters (0-9, a-f)");
        return false;
    }

    true
}

fn is_valid_sqm_id(id: &str) -> bool {
    // Format should be {XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX} (uppercase)
    if id.len() != 38 {
        println!("Warning: SQM ID format incorrect");
        return false;
    }

    if !id.starts_with('{') || !id.ends_with('}') {
        println!("Warning: SQM ID should be surrounded by curly braces");
        return false;
    }

    let uuid = &id[1..37];
    if !uuid
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
    {
        println!("Warning: UUID part should be uppercase letters, digits and hyphens");
        return false;
    }

    true
}

fn is_valid_device_id(id: &str) -> bool {
    // Format should be xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    if id.len() != 36 {
        println!("Warning: Device ID format incorrect");
        return false;
    }

    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        println!("Warning: Device ID should be lowercase letters, digits and hyphens");
        return false;
    }

    true
}

fn is_valid_email(email: &str) -> bool {
    if !email.contains('@') || !email.contains('.') {
        println!("Warning: Email format incorrect");
        return false;
    }
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        println!("Warning: Email format incorrect");
        return false;
    }
    true
}

fn is_valid_uuid(uuid: &str) -> bool {
    // UUID format should be: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    if uuid.len() != 36 {
        println!("Warning: UUID format incorrect");
        return false;
    }

    let parts: Vec<&str> = uuid.split('-').collect();
    if parts.len() != 5
        || parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        println!("Warning: UUID format incorrect");
        return false;
    }

    if !uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        println!("Warning: UUID should only contain hexadecimal characters (0-9, a-f) and hyphens");
        return false;
    }

    true
}

fn create_uuid_launcher(uuid: &str) -> io::Result<()> {
    let tools_dir = get_cursor_path().join("tools/set-token");
    fs::create_dir_all(&tools_dir)?;

    // Create inject.js
    let inject_js = format!(
        r#"// Save original require
const originalRequire = module.constructor.prototype.require;

// Override require function
module.constructor.prototype.require = function(path) {{
    const result = originalRequire.apply(this, arguments);
    
    // Detect target module
    if (path.includes('main.js')) {{
        // Save original function
        const originalModule = result;
        
        // Create proxy object
        return new Proxy(originalModule, {{
            get(target, prop) {{
                // Intercept execSync call
                if (prop === 'execSync') {{
                    return function() {{
                        // Return custom UUID
                        const platform = process.platform;
                        switch (platform) {{
                            case 'darwin':
                                return 'IOPlatformUUID="{}"';
                            case 'win32':
                                return '    HARDWARE\\DESCRIPTION\\System\\BIOS    SystemProductID    REG_SZ    {}';
                            case 'linux':
                            case 'freebsd':
                                return '{}';
                            default:
                                throw new Error(`Unsupported platform: ${{platform}}`);
                        }}
                    }};
                }}
                return target[prop];
            }}
        }});
    }}
    return result;
}};"#,
        uuid, uuid, uuid
    );

    // Write inject.js
    fs::write(tools_dir.join("inject.js"), inject_js)?;

    if cfg!(windows) {
        // Create Windows CMD script
        let cmd_script = format!(
            "@echo off\r\n\
            set NODE_OPTIONS=--require \"%~dp0inject.js\"\r\n\
            start \"\" \"%LOCALAPPDATA%\\Programs\\Cursor\\Cursor.exe\""
        );
        fs::write(tools_dir.join("start-cursor.cmd"), cmd_script)?;

        // Create Windows PowerShell script
        let ps_script = format!(
            "$env:NODE_OPTIONS = \"--require `\"$PSScriptRoot\\inject.js`\"\"\r\n\
            Start-Process -FilePath \"$env:LOCALAPPDATA\\Programs\\Cursor\\Cursor.exe\""
        );
        fs::write(tools_dir.join("start-cursor.ps1"), ps_script)?;
    } else {
        // Create Shell script
        let shell_script = format!(
            "#!/bin/bash\n\
            SCRIPT_DIR=\"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")\" && pwd)\"\n\
            export NODE_OPTIONS=\"--require $SCRIPT_DIR/inject.js\"\n\
            if [[ \"$OSTYPE\" == \"darwin\"* ]]; then\n\
                open -a Cursor\n\
            else\n\
                cursor # Linux, adjust according to actual installation path\n\
            fi"
        );
        let script_path = tools_dir.join("start-cursor.sh");
        fs::write(&script_path, shell_script)?;

        // Set executable permission on Unix-like systems
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }
    }

    println!("\nInjection script created at: {}", tools_dir.display());
    println!("\nUsage:");
    if cfg!(windows) {
        println!(
            "Method 1: Double-click to run {}",
            tools_dir.join("start-cursor.cmd").display()
        );
        println!(
            "Method 2: Run in PowerShell {}",
            tools_dir.join("start-cursor.ps1").display()
        );
    } else {
        println!(
            "Run in terminal: {}",
            tools_dir.join("start-cursor.sh").display()
        );
    }
    println!("\nNote: You need to use this script every time you start Cursor.");

    Ok(())
}

fn main() {
    loop {
        println!("\nPlease select operation:");
        println!("0. Exit");
        println!("1. Update Token");
        println!("2. Update Device ID");
        println!("3. Create custom UUID launch script");

        print!("Please enter option (0-3): ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "0" => break,
            "1" => {
                let mut refresh_token = String::new();
                loop {
                    print!("Please enter Refresh Token: ");
                    io::stdout().flush().unwrap();
                    refresh_token.clear();
                    io::stdin().read_line(&mut refresh_token).unwrap();
                    refresh_token = refresh_token.trim().to_string();

                    if is_valid_jwt(&refresh_token) {
                        break;
                    }
                    println!("Please re-enter Token in correct format");
                }

                print!("Is Access Token same as Refresh Token? (y/n): ");
                io::stdout().flush().unwrap();
                let mut same = String::new();
                io::stdin().read_line(&mut same).unwrap();

                let access_token = if same.trim().eq_ignore_ascii_case("y") {
                    refresh_token.clone()
                } else {
                    let mut access_token = String::new();
                    loop {
                        print!("Please enter Access Token: ");
                        io::stdout().flush().unwrap();
                        access_token.clear();
                        io::stdin().read_line(&mut access_token).unwrap();
                        access_token = access_token.trim().to_string();

                        if is_valid_jwt(&access_token) {
                            break;
                        }
                        println!("Please re-enter Token in correct format");
                    }
                    access_token
                };

                let mut email = String::new();
                loop {
                    print!("Please enter email: ");
                    io::stdout().flush().unwrap();
                    email.clear();
                    io::stdin().read_line(&mut email).unwrap();
                    email = email.trim().to_string();

                    if is_valid_email(&email) {
                        break;
                    }
                    println!("Please re-enter email in correct format");
                }

                let mut signup_type = String::new();
                loop {
                    println!("\nAvailable signup types:");
                    println!("1. Auth_0");
                    println!("2. Github");
                    println!("3. Google");
                    println!("4. unknown");
                    println!("(WorkOS - display only, not selectable)");
                    print!("Please select signup type (1-4): ");
                    io::stdout().flush().unwrap();
                    signup_type.clear();
                    io::stdin().read_line(&mut signup_type).unwrap();

                    let signup_type_str = match signup_type.trim() {
                        "1" => "Auth_0",
                        "2" => "Github",
                        "3" => "Google",
                        "4" => "unknown",
                        _ => continue,
                    }
                    .to_string();

                    signup_type = signup_type_str;
                    break;
                }

                let mut membership_type = String::new();
                loop {
                    println!("\nAvailable membership types:");
                    println!("1. free");
                    println!("2. pro");
                    println!("3. enterprise");
                    println!("4. free_trial");
                    print!("Please select membership type (1-4): ");
                    io::stdout().flush().unwrap();
                    membership_type.clear();
                    io::stdin().read_line(&mut membership_type).unwrap();

                    let membership_type_str = match membership_type.trim() {
                        "1" => "free",
                        "2" => "pro",
                        "3" => "enterprise",
                        "4" => "free_trial",
                        _ => continue,
                    }
                    .to_string();

                    membership_type = membership_type_str;
                    break;
                }

                match update_sqlite_tokens(
                    &refresh_token,
                    &access_token,
                    &email,
                    &signup_type,
                    &membership_type,
                ) {
                    Ok(_) => println!("All information updated successfully!"),
                    Err(e) => println!("Update failed: {}", e),
                }
            }
            "2" => {
                let mut ids = Vec::new();
                let validators: [(Box<dyn Fn(&str) -> bool>, &str); 4] = [
                    (Box::new(is_valid_sha256), "macMachineId"),
                    (Box::new(is_valid_sqm_id), "sqmId"),
                    (Box::new(is_valid_sha256), "machineId"),
                    (Box::new(is_valid_device_id), "devDeviceId"),
                ];

                for (validator, name) in validators.iter() {
                    loop {
                        print!("Please enter {}: ", name);
                        io::stdout().flush().unwrap();
                        let mut id = String::new();
                        io::stdin().read_line(&mut id).unwrap();
                        let id = id.trim().to_string();

                        if validator(&id) {
                            ids.push(id);
                            break;
                        }
                        println!("Please re-enter ID in correct format");
                    }
                }

                match update_storage_json(&ids.try_into().unwrap()) {
                    Ok(_) => println!("Device ID updated successfully!"),
                    Err(e) => println!("Update failed: {}", e),
                }
            }
            "3" => {
                let mut uuid = String::new();
                loop {
                    print!("Please enter custom UUID (format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx): ");
                    io::stdout().flush().unwrap();
                    uuid.clear();
                    io::stdin().read_line(&mut uuid).unwrap();
                    uuid = uuid.trim().to_string();

                    if is_valid_uuid(&uuid) {
                        break;
                    }
                    println!("Please re-enter UUID in correct format");
                }

                match create_uuid_launcher(&uuid) {
                    Ok(_) => println!("Launch script created successfully!"),
                    Err(e) => println!("Creation failed: {}", e),
                }
            }
            _ => println!("Invalid option, please try again"),
        }
    }
}
