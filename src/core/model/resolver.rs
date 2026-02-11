use super::Models;
use crate::{
    app::model::{AppConfig, UsageCheck},
    core::constant::get_static_id,
};
use byte_str::ByteStr;

static mut BYPASS_MODEL_VALIDATION: bool = false;

pub fn init_resolver() {
    unsafe {
        BYPASS_MODEL_VALIDATION =
            crate::common::utils::parse_from_env("BYPASS_MODEL_VALIDATION", false)
    }
}

#[derive(Clone, Copy)]
pub struct ExtModel {
    pub id: &'static str,
    pub is_image: bool,
    pub is_thinking: bool,
    pub web: bool,
    pub max: bool,
}

impl ExtModel {
    /// Parse ExtModel from string
    /// Support "-online" and "-max" suffix
    /// When BYPASS_MODEL_VALIDATION is true, still return result after normal validation fails
    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        // Handle online suffix
        let (base_str, web) = s.strip_suffix("-online").map_or((s, false), |base| (base, true));

        // Try direct matching first (may have -max suffix)
        if let Some(raw) = Models::find_id(base_str) {
            return Some(Self {
                id: raw.server_id,
                is_image: raw.is_image,
                is_thinking: raw.is_thinking,
                web,
                max: !raw.is_non_max,
            });
        }

        // Handle max suffix
        let (model_str, max) =
            base_str.strip_suffix("-max").map_or((base_str, false), |base| (base, true));

        // If has -max suffix, try matching model name without suffix
        if max
            && let Some(raw) = Models::find_id(model_str)
            && raw.is_max
        {
            return Some(Self {
                id: raw.server_id,
                is_image: raw.is_image,
                is_thinking: raw.is_thinking,
                web,
                max,
            });
        }

        // After all normal validations failed, check whether to bypass validation
        if unsafe { BYPASS_MODEL_VALIDATION } && !model_str.is_empty() {
            let id = get_static_id(model_str);
            return Some(Self {
                id,
                is_image: true,
                // The detection here is weak, however, the impact may not be significant
                is_thinking: id.contains("-thinking")
                    || {
                        let bytes = id.as_bytes();
                        bytes.len() >= 2 && bytes[0] == b'o' && bytes[1].is_ascii_digit()
                    }
                    || id.starts_with("gemini-2.5-")
                    || id.starts_with("deepseek-r1")
                    || id.starts_with("grok-4"),
                web,
                max,
            });
        }

        None
    }

    pub fn is_usage_check(&self, usage_check: Option<UsageCheck>) -> bool {
        usage_check.unwrap_or(AppConfig::model_usage_checks()).check(&self.id)
    }

    #[inline(always)]
    pub const fn id(&self) -> ByteStr { ByteStr::from_static(self.id) }
}
