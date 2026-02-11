use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn main() -> std::io::Result<()> {
    // Get user home directory path
    let home_dir = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap();

    // Build storage.json path
    let db_path = if cfg!(target_os = "windows") {
        PathBuf::from(home_dir.clone())
            .join(r"AppData\Roaming\Cursor\User\globalStorage\storage.json")
    } else if cfg!(target_os = "linux") {
        PathBuf::from(home_dir.clone()).join(".config/Cursor/User/globalStorage/storage.json")
    } else {
        PathBuf::from(home_dir.clone())
            .join("Library/Application Support/Cursor/User/globalStorage/storage.json")
    };

    // Build machineid file path
    let machine_id_path = if cfg!(target_os = "windows") {
        PathBuf::from(home_dir).join(r"AppData\Roaming\Cursor\machineid")
    } else if cfg!(target_os = "linux") {
        PathBuf::from(home_dir).join(".config/Cursor/machineid")
    } else {
        PathBuf::from(home_dir).join("Library/Application Support/Cursor/machineid")
    };

    // Read and update storage.json
    let mut content: Value = if db_path.exists() {
        let content = fs::read_to_string(&db_path)?;
        serde_json::from_str(&content)?
    } else {
        json!({})
    };

    // Generate new telemetry IDs
    content["telemetry.macMachineId"] = json!(generate_sha256_hash());
    content["telemetry.sqmId"] = json!(generate_sqm_id());
    content["telemetry.machineId"] = json!(generate_sha256_hash());
    content["telemetry.devDeviceId"] = json!(generate_device_id());

    // Write updated storage.json
    fs::write(&db_path, serde_json::to_string_pretty(&content)?)?;

    // Update machineid file
    fs::write(&machine_id_path, generate_device_id())?;

    println!("Telemetry IDs reset successfully!");
    Ok(())
}

fn generate_sha256_hash() -> String {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    let hash = Sha256::digest(&bytes);
    format!("{:x}", hash)
}

fn generate_sqm_id() -> String {
    use hex::ToHex as _;
    Uuid::new_v4().braced().encode_hex_upper()
}

fn generate_device_id() -> String {
    Uuid::new_v4().to_string()
}
