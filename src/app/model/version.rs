use core::cmp::Ordering;
use std::io;

/// Release stage
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
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

impl PartialOrd for ReleaseStage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for ReleaseStage {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Preview < Release
            (ReleaseStage::Preview { .. }, ReleaseStage::Release) => Ordering::Less,
            (ReleaseStage::Release, ReleaseStage::Preview { .. }) => Ordering::Greater,

            // Release versions are equal
            (ReleaseStage::Release, ReleaseStage::Release) => Ordering::Equal,

            // Preview versions: compare version first, then build
            (
                ReleaseStage::Preview { version: v1, build: b1 },
                ReleaseStage::Preview { version: v2, build: b2 },
            ) => v1.cmp(v2).then_with(|| b1.cmp(b2)),
        }
    }
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

/// Follow format: v0.4.0-pre.6+build.8
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub stage: ReleaseStage,
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare major -> minor -> patch -> stage in order
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(|| self.stage.cmp(&other.stage))
    }
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}{}", self.major, self.minor, self.patch, self.stage)
    }
}

impl Version {
    /// Write to writer
    ///
    /// Binary format (use native byte order):
    /// - [0-1] major: u16
    /// - [2-3] minor: u16
    /// - [4-5] patch: u16
    /// - [6-7] len: u16 (0=Release, 1=Preview, 2=PreviewBuild)
    /// - [8-9] (optional) pre_version: u16
    /// - [10-11] (optional) build: u16
    ///
    /// # Errors
    ///
    /// If write fails, return I/O error
    pub fn write_to<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        // Write fixed header
        writer.write_all(&self.major.to_ne_bytes())?;
        writer.write_all(&self.minor.to_ne_bytes())?;
        writer.write_all(&self.patch.to_ne_bytes())?;

        // Write based on stage, len and metadata
        match self.stage {
            ReleaseStage::Release => {
                writer.write_all(&0u16.to_ne_bytes())?;
            }
            ReleaseStage::Preview { version, build: None } => {
                writer.write_all(&1u16.to_ne_bytes())?;
                writer.write_all(&version.to_ne_bytes())?;
            }
            ReleaseStage::Preview { version, build: Some(build) } => {
                writer.write_all(&2u16.to_ne_bytes())?;
                writer.write_all(&version.to_ne_bytes())?;
                writer.write_all(&build.to_ne_bytes())?;
            }
        }

        Ok(())
    }

    /// Read from reader
    ///
    /// # Errors
    ///
    /// - `UnexpectedEof`: insufficient data
    /// - `InvalidData`: len value is invalid (>2)
    /// - other I/O errors
    pub fn read_from<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0u8; 2];

        // Read fixed header
        reader.read_exact(&mut buf)?;
        let major = u16::from_ne_bytes(buf);

        reader.read_exact(&mut buf)?;
        let minor = u16::from_ne_bytes(buf);

        reader.read_exact(&mut buf)?;
        let patch = u16::from_ne_bytes(buf);

        reader.read_exact(&mut buf)?;
        let len = u16::from_ne_bytes(buf);

        // Read metadata based on len
        let stage = match len {
            0 => ReleaseStage::Release,
            1 => {
                reader.read_exact(&mut buf)?;
                let version = u16::from_ne_bytes(buf);
                ReleaseStage::Preview { version, build: None }
            }
            2 => {
                reader.read_exact(&mut buf)?;
                let version = u16::from_ne_bytes(buf);
                reader.read_exact(&mut buf)?;
                let build = u16::from_ne_bytes(buf);
                ReleaseStage::Preview { version, build: Some(build) }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid version length field: {len}"),
                ));
            }
        };

        Ok(Version { major, minor, patch, stage })
    }
}

// Helper function: create official version
#[allow(dead_code)]
pub const fn release(major: u16, minor: u16, patch: u16) -> Version {
    Version { major, minor, patch, stage: ReleaseStage::Release }
}

// 辅助函数：创建预览版本（无 build）
#[allow(dead_code)]
pub const fn preview(major: u16, minor: u16, patch: u16, version: u16) -> Version {
    Version { major, minor, patch, stage: ReleaseStage::Preview { version, build: None } }
}

// 辅助函数：创建预览版本（带 build）
#[allow(dead_code)]
pub const fn preview_build(
    major: u16,
    minor: u16,
    patch: u16,
    version: u16,
    build: u16,
) -> Version {
    Version { major, minor, patch, stage: ReleaseStage::Preview { version, build: Some(build) } }
}
