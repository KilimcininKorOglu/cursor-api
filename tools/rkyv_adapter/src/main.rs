use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    path::PathBuf,
    sync::LazyLock,
};

use chrono::{DateTime, Local};
use memmap2::{MmapMut, MmapOptions};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

#[derive(Clone, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct TokenInfo {
    pub alias: Option<String>,
    pub token: String,
    pub checksum: Checksum,
    pub status: TokenStatus,
    pub client_key: Hash,
    pub config_version: Option<uuid::Uuid>,
    pub session_id: Option<uuid::Uuid>,
    pub profile: Option<TokenProfile>,
    pub tags: Option<HashMap<String, Option<String>>>,
}

#[derive(Clone, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct OldTokenInfo {
    pub token: String,
    pub checksum: Checksum,
    pub status: OldTokenStatus,
    pub client_key: Hash,
    pub config_version: Option<uuid::Uuid>,
    pub session_id: Option<uuid::Uuid>,
    pub profile: Option<TokenProfile>,
    pub tags: Option<HashMap<String, Option<String>>>,
}

impl From<OldTokenInfo> for TokenInfo {
    fn from(value: OldTokenInfo) -> Self {
        Self {
            alias: if let Some(profile) = &value.profile {
                Some(profile.user.email.clone())
            } else {
                None
            },
            token: value.token,
            checksum: value.checksum,
            status: value.status.into(),
            client_key: value.client_key,
            config_version: value.config_version,
            session_id: value.session_id,
            profile: value.profile,
            tags: value.tags,
        }
    }
}

#[derive(Clone, Copy, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct Checksum {
    first: Hash,
    second: Hash,
}

#[derive(Default, Clone, Copy, Archive, RkyvSerialize, RkyvDeserialize)]
#[repr(u8)]
pub enum OldTokenStatus {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Default, Clone, Copy, Archive, RkyvSerialize, RkyvDeserialize)]
#[repr(u8)]
pub enum TokenStatus {
    #[default]
    Enabled,
    Disabled,
    Hidden,
}

impl From<OldTokenStatus> for TokenStatus {
    fn from(value: OldTokenStatus) -> Self {
        match value {
            OldTokenStatus::Enabled => TokenStatus::Enabled,
            OldTokenStatus::Disabled => TokenStatus::Disabled,
        }
    }
}

#[derive(Clone, Copy, Archive, RkyvSerialize, RkyvDeserialize)]
#[repr(transparent)]
pub struct Hash([u8; 32]);

#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct TokenProfile {
    pub usage: UsageProfile,
    pub user: UserProfile,
    pub stripe: StripeProfile,
}

#[derive(PartialEq, Clone, Copy, Archive, RkyvDeserialize, RkyvSerialize)]
pub enum MembershipType {
    Free,
    FreeTrial,
    Pro,
    Enterprise,
}

impl std::str::FromStr for MembershipType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "free" => Ok(MembershipType::Free),
            "free_trial" => Ok(MembershipType::FreeTrial),
            "pro" => Ok(MembershipType::Pro),
            "enterprise" => Ok(MembershipType::Enterprise),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct StripeProfile {
    pub membership_type: MembershipType,
    pub payment_id: Option<String>,
    pub days_remaining_on_trial: u32,
}

#[derive(Clone, Copy, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct ModelUsage {
    pub num_requests: u32,
    pub total_requests: Option<u32>,
    pub num_tokens: u32,
    pub max_requests: Option<u32>,
    pub max_tokens: Option<u32>,
}

#[derive(Clone, Copy, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct UsageProfile {
    pub premium: ModelUsage,
    pub standard: ModelUsage,
    pub unknown: ModelUsage,
    pub start_of_month: DateTime<Local>,
}

#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct UserProfile {
    pub email: String,
    // pub email_verified: bool,
    pub name: String,
    pub sub: String,
    pub updated_at: DateTime<Local>,
    // Image link, rendered in /logs?
    // pub picture: Option<String>,
}

#[derive(Archive, RkyvDeserialize, RkyvSerialize)]
enum ErrorInfoHelper {
    None,
    Error(String),
    Details { error: String, details: String },
}
#[derive(Archive, RkyvDeserialize, RkyvSerialize)]
pub struct RequestLogHelper {
    id: u64,
    timestamp: chrono::DateTime<chrono::Local>,
    model: String,
    token_info: TokenInfo,
    chain: Option<ChainHelper>,
    timing: TimingInfo,
    stream: bool,
    status: LogStatus,
    error: ErrorInfoHelper,
}
#[derive(Archive, RkyvDeserialize, RkyvSerialize)]
pub struct OldRequestLogHelper {
    id: u64,
    timestamp: chrono::DateTime<chrono::Local>,
    model: String,
    token_info: OldTokenInfo,
    chain: Option<ChainHelper>,
    timing: TimingInfo,
    stream: bool,
    status: LogStatus,
    error: ErrorInfoHelper,
}
impl From<OldRequestLogHelper> for RequestLogHelper {
    fn from(value: OldRequestLogHelper) -> Self {
        Self {
            id: value.id,
            timestamp: value.timestamp,
            model: value.model,
            token_info: {
                let value = value.token_info;
                TokenInfo {
                    alias: None,
                    token: value.token,
                    checksum: value.checksum,
                    status: value.status.into(),
                    client_key: value.client_key,
                    config_version: value.config_version,
                    session_id: value.session_id,
                    profile: value.profile,
                    tags: value.tags,
                }
            },
            chain: value.chain,
            timing: value.timing,
            stream: value.stream,
            status: value.status,
            error: value.error,
        }
    }
}

impl OldRequestLogHelper {
    fn load_logs() -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let file = match OpenOptions::new().read(true).open(&*LOGS_FILE_PATH) {
            Ok(file) => file,
            Err(e) => return Err(Box::new(e)),
        };

        if file.metadata()?.len() > usize::MAX as u64 {
            return Err("Log file too large".into());
        }

        let mmap = unsafe { MmapOptions::new().map(&file)? };
        let archived = unsafe { rkyv::archived_root::<Vec<Self>>(&mmap) };
        let helper: Vec<Self> = archived.deserialize(&mut rkyv::Infallible)?;

        Ok(helper)
    }
}

impl RequestLogHelper {
    fn save_logs(logs: Vec<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = rkyv::to_bytes::<_, 256>(&logs)?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*LOGS_FILE_PATH)?;

        if bytes.len() > usize::MAX >> 1 {
            return Err("Log data too large".into());
        }

        file.set_len(bytes.len() as u64)?;
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        mmap.copy_from_slice(&bytes);
        mmap.flush()?;

        Ok(())
    }
}
#[derive(Clone, Copy, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct TimingInfo {
    pub total: f64, // Total time (seconds)
}
#[derive(Clone, Copy, PartialEq, Archive, RkyvDeserialize, RkyvSerialize)]
#[repr(u8)]
pub enum LogStatus {
    Pending,
    Success,
    Failure,
}
#[derive(Archive, RkyvDeserialize, RkyvSerialize)]
pub struct PromptMessageHelper {
    role: Role,
    content: String,
}
#[derive(Archive, RkyvDeserialize, RkyvSerialize, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Role {
    System = 0u8,
    User,
    Assistant,
}
#[derive(Archive, RkyvDeserialize, RkyvSerialize)]
pub enum PromptHelper {
    None,
    Origin(String),
    Parsed(Vec<PromptMessageHelper>),
}
#[derive(Archive, RkyvDeserialize, RkyvSerialize)]
pub struct ChainHelper {
    pub prompt: PromptHelper,
    pub delays: Option<(String, Vec<(u32, f32)>)>,
    pub usage: OptionUsage,
    pub think: Option<String>,
}
#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub enum OptionUsage {
    None,
    Uasge { input: i32, output: i32 },
}
#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct TokenManager {
    pub tokens: Vec<TokenInfo>,
    pub aliases: HashSet<String>,
    pub tags: HashSet<String>,
}
#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct OldTokenManager {
    pub tokens: Vec<OldTokenInfo>,
    pub tags: HashSet<String>,
}
impl From<OldTokenManager> for TokenManager {
    fn from(value: OldTokenManager) -> Self {
        let tokens: Vec<TokenInfo> = value.tokens.into_iter().map(Into::into).collect();
        let mut aliases = HashSet::new();
        let mut tags = HashSet::new();
        for token in &tokens {
            if let Some(token_tags) = &token.tags {
                tags.extend(token_tags.keys().cloned());
            }
            if let Some(alias) = &token.alias {
                aliases.insert(alias.clone());
            }
        }
        Self {
            tokens,
            aliases,
            tags,
        }
    }
}

impl OldTokenManager {
    fn load_tokens() -> Result<Self, Box<dyn std::error::Error>> {
        let file = match OpenOptions::new().read(true).open(&*TOKENS_FILE_PATH) {
            Ok(file) => file,
            Err(e) => return Err(Box::new(e)),
        };

        if file.metadata()?.len() > usize::MAX as u64 {
            return Err("Token file too large".into());
        }

        let mmap = unsafe { MmapOptions::new().map(&file)? };
        let archived = unsafe { rkyv::archived_root::<Self>(&mmap) };
        Ok(archived.deserialize(&mut rkyv::Infallible)?)
    }
}

impl TokenManager {
    fn save_tokens(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = rkyv::to_bytes::<_, 256>(self)?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*TOKENS_FILE_PATH)?;

        if bytes.len() > usize::MAX >> 1 {
            return Err("Token data too large".into());
        }

        file.set_len(bytes.len() as u64)?;
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        mmap.copy_from_slice(&bytes);
        mmap.flush()?;

        Ok(())
    }
}

static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let path = std::env::current_exe()
        .ok()
        .and_then(|exe_path| exe_path.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(data_dir);
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Unable to create data directory");
    }
    path
});

static LOGS_FILE_PATH: LazyLock<PathBuf> = LazyLock::new(|| DATA_DIR.join("logs.bin"));

static TOKENS_FILE_PATH: LazyLock<PathBuf> = LazyLock::new(|| DATA_DIR.join("tokens.bin"));

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set custom panic hook
    std::panic::set_hook(Box::new(|info| {
        if let Some(msg) = info.payload().downcast_ref::<String>() {
            eprintln!("{msg}");
        } else if let Some(msg) = info.payload().downcast_ref::<&str>() {
            eprintln!("{msg}");
        }
    }));

    // Load environment variables
    dotenvy::dotenv().ok();

    // Add interactive confirmation
    println!("Are you sure to use data adapter (cursor-api auxiliary tool) to migrate from v0.2.8 to v0.2.9? (This operation is irreversible)");

    let mut input = String::new();
    println!(
        "Please enter 'y'/'yes' to confirm or 'n'/'no' to cancel:"
    );
    std::io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    if input != "y" && input != "yes" {
        println!("Operation cancelled");
        return Ok(());
    }

    // Execute migration
    let old = OldTokenManager::load_tokens()?;
    let new: TokenManager = old.into();
    new.save_tokens()?;
    let old = OldRequestLogHelper::load_logs()?;
    let new: Vec<RequestLogHelper> = old.into_iter().map(Into::into).collect();
    RequestLogHelper::save_logs(new)?;

    println!("Data migration completed successfully");
    Ok(())
}
