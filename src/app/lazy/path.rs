use crate::common::utils::parse_from_env;
use manually_init::ManuallyInit;
use std::path::{Path, PathBuf};

#[inline]
pub fn create_dir_all<P: AsRef<Path>>(path: P) {
    std::fs::create_dir_all(path).expect("Unable to create data directory")
}

pub fn init(current_dir: PathBuf) {
    CURRENT_DIR.init(current_dir);
    CONFIG_FILE_PATH.init(CURRENT_DIR.join(&*parse_from_env("CONFIG_FILE", "config.toml")));
    DATA_DIR.init({
        let data_dir = parse_from_env("DATA_DIR", "data");
        let path = CURRENT_DIR.join(&*data_dir);
        if !path.exists() {
            create_dir_all(&path)
        }
        path
    });
    LOGS_DIR.init({
        let logs_dir = parse_from_env("LOGS_DIR", "logs");
        let path = DATA_DIR.join(&*logs_dir);
        if !path.exists() {
            create_dir_all(&path)
        }
        path
    });
    LOGS_FILE_PATH.init(DATA_DIR.join("logs.bin"));
    TOKENS_FILE_PATH.init(DATA_DIR.join("tokens.bin"));
    PROXIES_FILE_PATH.init(DATA_DIR.join("proxies.bin"));
}

pub static CURRENT_DIR: ManuallyInit<PathBuf> = ManuallyInit::new();

pub static CONFIG_FILE_PATH: ManuallyInit<PathBuf> = ManuallyInit::new();

pub static DATA_DIR: ManuallyInit<PathBuf> = ManuallyInit::new();

pub static LOGS_DIR: ManuallyInit<PathBuf> = ManuallyInit::new();

pub static LOGS_FILE_PATH: ManuallyInit<PathBuf> = ManuallyInit::new();
pub static TOKENS_FILE_PATH: ManuallyInit<PathBuf> = ManuallyInit::new();
pub static PROXIES_FILE_PATH: ManuallyInit<PathBuf> = ManuallyInit::new();
