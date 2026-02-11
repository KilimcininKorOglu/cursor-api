//! 提供时间有效期范围的数据结构andRelated操作

use std::num::ParseIntError;

/// 表示一个有效期范围，以两个u32值表示起始and结束时间。
///
/// 该结构体Use透明内存布局，通过[u32; 2]实现8字节Size。
/// Support从字符串解析，比如"60"表示60-60的范围，"3600-86400"表示3600到86400的闭区间。
#[repr(transparent)]
pub struct ValidityRange {
    range: [u32; 2], // range[0]Tostart，range[1]Toend
}

// 验证内存布局约束
const _: [u8; 8] = [0; ::core::mem::size_of::<ValidityRange>()]; // EnsureSizeTo8字节

impl ValidityRange {
    /// 创建新的有效期范围实例
    ///
    /// # 参数
    ///
    /// * `start` - 范围的起始值
    /// * `end` - 范围的结束值
    ///
    /// # 示例
    ///
    /// ```
    /// let range = ValidityRange::new(60, 3600);
    /// ```
    #[inline]
    pub const fn new(start: u32, end: u32) -> Self {
        ValidityRange {
            range: [start, end],
        }
    }

    /// Get范围的起始值
    #[inline(always)]
    pub const fn start(&self) -> u32 { self.range[0] }

    /// Get范围的结束值
    #[inline(always)]
    pub const fn end(&self) -> u32 { self.range[1] }

    /// 检查给定值是否在有效期范围内
    ///
    /// # 参数
    ///
    /// * `value` - 待检查的值
    ///
    /// # 返回值
    ///
    /// If值在范围内（包括边界值）返回true，否则返回false
    ///
    /// # 示例
    ///
    /// ```
    /// let range = ValidityRange::new(60, 3600);
    /// assert!(range.is_valid(60));
    /// assert!(range.is_valid(3600));
    /// assert!(range.is_valid(1800));
    /// assert!(!range.is_valid(59));
    /// assert!(!range.is_valid(3601));
    /// ```
    #[inline]
    pub const fn is_valid(&self, value: u32) -> bool {
        value >= self.start() && value <= self.end()
    }

    /// 从字符串解析有效期范围
    ///
    /// Support两种Format：
    /// - "N" 表示N-N的单点范围
    /// - "N-M" 表示从N到M的范围
    ///
    /// # 参数
    ///
    /// * `s` - 要解析的字符串
    ///
    /// # 返回值
    ///
    /// 成功解析返回`Ok(ValidityRange)`，Failed返回包含Error类型的`Err`
    ///
    /// # 示例
    ///
    /// ```
    /// let range1 = ValidityRange::from_str("60").unwrap();
    /// assert_eq!(range1.start(), 60);
    /// assert_eq!(range1.end(), 60);
    ///
    /// let range2 = ValidityRange::from_str("3600-86400").unwrap();
    /// assert_eq!(range2.start(), 3600);
    /// assert_eq!(range2.end(), 86400);
    /// ```
    pub fn from_str(s: &str) -> Result<Self, ParseIntError> {
        if let Some((start_str, end_str)) = s.split_once('-') {
            let start = start_str.parse::<u32>()?;
            let end = end_str.parse::<u32>()?;

            Ok(ValidityRange::new(start, end))
        } else {
            // Format: "value" (表示value-value)
            let value = s.parse::<u32>()?;
            Ok(ValidityRange::new(value, value))
        }
    }
}

/// 实现Display特性，用于Format化输出
///
/// 对于相同的起始and结束值，只显示一个数字；
/// 对于不同的值，显示To"start-end"Format。
impl std::fmt::Display for ValidityRange {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let start = self.start();
        let end = self.end();

        if start == end {
            write!(f, "{start}")
        } else {
            write!(f, "{start}-{end}")
        }
    }
}

/// 实现Debug特性，提供更详细的Format化输出
impl std::fmt::Debug for ValidityRange {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ValidityRange({}-{})", self.start(), self.end())
    }
}

/// 实现FromStr特性，Support从字符串解析
///
/// 这使得可以直接Use`str.parse()`方法解析字符串ToValidityRange
impl std::str::FromStr for ValidityRange {
    type Err = ParseIntError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> { ValidityRange::from_str(s) }
}

/// 单元测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let range = ValidityRange::new(60, 3600);
        assert_eq!(range.start(), 60);
        assert_eq!(range.end(), 3600);
    }

    #[test]
    fn test_is_valid() {
        let range = ValidityRange::new(60, 3600);
        assert!(range.is_valid(60));
        assert!(range.is_valid(3600));
        assert!(range.is_valid(1800));
        assert!(!range.is_valid(59));
        assert!(!range.is_valid(3601));
    }

    #[test]
    fn test_from_str_single() {
        let range = ValidityRange::from_str("60").unwrap();
        assert_eq!(range.start(), 60);
        assert_eq!(range.end(), 60);
    }

    #[test]
    fn test_from_str_range() {
        let range = ValidityRange::from_str("3600-86400").unwrap();
        assert_eq!(range.start(), 3600);
        assert_eq!(range.end(), 86400);
    }

    #[test]
    fn test_from_str_invalid() {
        assert!(ValidityRange::from_str("abc").is_err());
        assert!(ValidityRange::from_str("123-abc").is_err());
        assert!(ValidityRange::from_str("abc-123").is_err());
    }

    #[test]
    fn test_display() {
        let range1 = ValidityRange::new(60, 60);
        assert_eq!(format!("{range1}"), "60");

        let range2 = ValidityRange::new(3600, 86400);
        assert_eq!(format!("{range2}"), "3600-86400");
    }

    #[test]
    fn test_debug() {
        let range = ValidityRange::new(60, 3600);
        assert_eq!(format!("{range:?}"), "ValidityRange(60-3600)");
    }

    #[test]
    fn test_parse() {
        let range: Result<ValidityRange, _> = "60".parse();
        assert!(range.is_ok());
        let range = range.unwrap();
        assert_eq!(range.start(), 60);
        assert_eq!(range.end(), 60);
    }
}
