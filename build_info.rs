include!("src/app/model/version.rs");

/// Version string parse error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// Overall format error (e.g., missing required parts)
    InvalidFormat,
    /// Number parsing failed
    InvalidNumber,
    /// Pre-release part format error
    InvalidPreRelease,
    /// Build part format error
    InvalidBuild,
    // /// Release version cannot have build identifier
    // BuildWithoutPreview,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::InvalidFormat => write!(f, "invalid version format"),
            ParseError::InvalidNumber => write!(f, "invalid number in version"),
            ParseError::InvalidPreRelease => write!(f, "invalid pre-release format"),
            ParseError::InvalidBuild => write!(f, "invalid build format"),
            // ParseError::BuildWithoutPreview => {
            //     write!(f, "build metadata cannot exist without pre-release version")
            // }
        }
    }
}

impl std::error::Error for ParseError {}

impl core::str::FromStr for Version {
    type Err = ParseError;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        // Split base version number and extension part by '-'
        let (base, extension) = match s.split_once('-') {
            Some((base, ext)) => (base, Some(ext)),
            None => (s, None),
        };

        // Parse base version number major.minor.patch
        let mut parts: [u16; 3] = [0, 0, 0];
        let mut parsed_count = 0;
        for (i, s) in base.split('.').enumerate() {
            if i >= parts.len() {
                return Err(ParseError::InvalidFormat);
            }
            parts[i] = s.parse().map_err(|_| ParseError::InvalidNumber)?;
            parsed_count += 1;
        }
        if parsed_count != 3 {
            return Err(ParseError::InvalidFormat);
        }

        let major = parts[0];
        let minor = parts[1];
        let patch = parts[2];

        // Parse extension part (if exists)
        let stage =
            if let Some(ext) = extension { parse_extension(ext)? } else { ReleaseStage::Release };

        Ok(Version { major, minor, patch, stage })
    }
}

/// Parse extension part: pre.X or pre.X+build.Y
fn parse_extension(s: &str) -> core::result::Result<ReleaseStage, ParseError> {
    // Check if starts with "pre."
    if !s.starts_with("pre.") {
        return Err(ParseError::InvalidPreRelease);
    }

    // Remove "pre." prefix
    let after_pre = &s[4..];

    // Split version and build parts by '+'
    let (version_str, build_str) = match after_pre.split_once('+') {
        Some((ver, build_part)) => (ver, Some(build_part)),
        None => (after_pre, None),
    };

    // Parse pre version number
    let version = version_str.parse().map_err(|_| ParseError::InvalidPreRelease)?;

    // Parse build number (if exists)
    let build = if let Some(build_part) = build_str {
        // Check if format is "build.X"
        if !build_part.starts_with("build.") {
            return Err(ParseError::InvalidBuild);
        }

        let build_num_str = &build_part[6..];
        let build_num = build_num_str.parse().map_err(|_| ParseError::InvalidBuild)?;

        Some(build_num)
    } else {
        None
    };

    Ok(ReleaseStage::Preview { version, build })
}

/**
 * Update version number function
 * This function reads the number from VERSION file, increments it by 1, then saves back to file
 * If VERSION file doesn't exist or is empty, counting starts from 1
 * Only executes in release mode, completely skipped in debug/dev mode
 */
#[cfg(not(debug_assertions))]
#[cfg(feature = "__preview")]
fn update_version() -> Result<()> {
    let version_path = "VERSION";
    // VERSION file monitoring is already added in main function, no need to repeat here

    // Read current version number
    let mut version = String::new();
    let mut file = match File::open(version_path) {
        Ok(file) => file,
        Err(_) => {
            // If file doesn't exist or can't be opened, start from 1
            println!("cargo:warning=VERSION file not found, creating with initial value 1");
            let mut new_file = File::create(version_path)?;
            new_file.write_all(b"1")?;
            return Ok(());
        }
    };

    file.read_to_string(&mut version)?;

    // Ensure version number is a valid number
    #[allow(unused_variables)]
    let version_num = match version.trim().parse::<u64>() {
        Ok(num) => num,
        Err(_) => {
            println!("cargo:warning=Invalid version number in VERSION file. Setting to 1.");
            let mut file = File::create(version_path)?;
            file.write_all(b"1")?;
            return Ok(());
        }
    };

    #[cfg(not(feature = "__preview_locked"))]
    {
        // Increment version number by 1
        let new_version = version_num + 1;
        println!(
            "cargo:warning=Release build - bumping version from {version_num} to {new_version}",
        );

        // Write back to file
        let mut file = File::create(version_path)?;
        write!(file, "{new_version}")?;
    }

    Ok(())
}

#[allow(unused)]
fn read_version_number() -> Result<u64> {
    let mut version = String::with_capacity(4);
    match std::fs::File::open("VERSION") {
        Ok(mut file) => {
            use std::io::Read as _;
            file.read_to_string(&mut version)?;
            Ok(version.trim().parse().unwrap_or(1))
        }
        Err(_) => Ok(1),
    }
}

fn generate_build_info() -> Result<()> {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_info.rs");
    // #[cfg(debug_assertions)]
    // let out_dir = "../target/debug/build/build_info.rs";
    // #[cfg(not(debug_assertions))]
    // let out_dir = "../target/release/build/build_info.rs";
    // let dest_path = Path::new(out_dir);
    // if dest_path.is_file() {
    //     return Ok(());
    // }

    let build_timestamp = std::env::var_os("BUILD_TIMESTAMP")
        .and_then(|s| s.to_str().and_then(|s| s.parse().ok()))
        .unwrap_or_else(|| {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
        });

    let build_timestamp_str = chrono::DateTime::from_timestamp(build_timestamp as i64, 0)
        .unwrap()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let pkg_version = env!("CARGO_PKG_VERSION");

    let (version_str, build_version_str) =
        if cfg!(feature = "__preview") && pkg_version.contains("-pre") {
            let build_num = read_version_number()?;
            (
                format!("{pkg_version}+build.{build_num}"),
                format!("pub const BUILD_VERSION: u32 = {build_num};\n"),
            )
        } else {
            (pkg_version.to_string(), String::new())
        };

    let version: Version = version_str.parse().unwrap();

    let build_info_content = format!(
        r#"// This file is auto-generated by build.rs, do not modify manually
use crate::app::model::version::{{Version, ReleaseStage::Preview}};

{build_version_str}pub const BUILD_TIMESTAMP: &'static str = {build_timestamp_str:?};
/// pub const VERSION_STR: &'static str = {version_str:?};
pub const VERSION: Version = {version:?};
pub const IS_PRERELEASE: bool = {is_prerelease};
pub const IS_DEBUG: bool = {is_debug};

#[cfg(unix)]
pub const BUILD_EPOCH: std::time::SystemTime =
    unsafe {{ ::core::intrinsics::transmute(({build_timestamp}i64, 0u32)) }};

#[cfg(windows)]
pub const BUILD_EPOCH: std::time::SystemTime = unsafe {{
    const INTERVALS_PER_SEC: u64 = 10_000_000;
    const INTERVALS_TO_UNIX_EPOCH: u64 = 11_644_473_600 * INTERVALS_PER_SEC;
    const TARGET_INTERVALS: u64 = INTERVALS_TO_UNIX_EPOCH + {build_timestamp} * INTERVALS_PER_SEC;

    ::core::intrinsics::transmute((
        TARGET_INTERVALS as u32,
        (TARGET_INTERVALS >> 32) as u32,
    ))
}};
"#,
        is_prerelease = cfg!(feature = "__preview"),
        is_debug = cfg!(debug_assertions),
    );

    std::fs::write(dest_path, build_info_content)?;
    Ok(())
}
