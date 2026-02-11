/// Version release stage
#[derive(Debug, Clone, Copy)]
pub enum ReleaseStage {
  /// Official release version
  Release,
  /// Preview version, format like `-pre.6` or `-pre.6+build.8`
  Preview {
    /// Preview version number
    version: u16,
    /// Build number (optional)
    build: Option<u16>,
  },
}

impl core::fmt::Display for ReleaseStage {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      ReleaseStage::Release => Ok(()),
      ReleaseStage::Preview { version, build: None } => {
        write!(f, "-pre.{version}")
      }
      ReleaseStage::Preview { version, build: Some(build) } => {
        write!(f, "-pre.{version}+build.{build}")
      }
    }
  }
}

/// Follows format: v0.4.0-pre.6+build.8
#[derive(Debug, Clone, Copy)]
pub struct Version {
  pub major: u16,
  pub minor: u16,
  pub patch: u16,
  pub stage: ReleaseStage,
}

impl core::fmt::Display for Version {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
    self.stage.fmt(f)
  }
}

/// Version string parse error
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
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
  // Remove "pre." prefix
  let Some(after_pre) = s.strip_prefix("pre.") else {
    return Err(ParseError::InvalidPreRelease);
  };

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
    let Some(build_num_str) = build_part.strip_prefix("build.") else {
      return Err(ParseError::InvalidBuild);
    };

    let build_num = build_num_str.parse().map_err(|_| ParseError::InvalidBuild)?;

    Some(build_num)
  } else {
    None
  };

  Ok(ReleaseStage::Preview { version, build })
}
