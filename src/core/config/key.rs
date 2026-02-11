use minicbor::{CborLen, Decode, Encode};

/// Dynamic configured API KEY
#[derive(Clone, PartialEq, Encode, Decode, CborLen)]
pub struct ConfiguredKey {
    /// Authentication token (required)
    #[n(0)]
    pub token_info: Option<configured_key::TokenInfo>,
    /// Password SHA256 hash value
    #[n(1)]
    pub secret: Option<[u8; 32]>,
    /// Whether to disable image processing capability
    #[n(2)]
    pub disable_vision: Option<bool>,
    /// Whether to enable slow pool
    #[n(3)]
    pub enable_slow_pool: Option<bool>,
    /// Include web references
    #[n(4)]
    pub include_web_references: Option<bool>,
    /// Usage check model rules
    #[n(5)]
    pub usage_check_models: Option<configured_key::UsageCheckModel>,
}

pub mod configured_key {
    use super::*;

    /// Authentication token information
    #[derive(Clone, PartialEq, Encode, Decode, CborLen)]
    pub struct TokenInfo {
        /// Token (required)
        #[n(0)]
        pub token: token_info::Token,
        /// Checksum (\[u8; 64\])
        #[n(1)]
        pub checksum: [u8; 64],
        /// Client identifier (\[u8; 32\])
        #[n(2)]
        pub client_key: [u8; 32],
        /// Configuration version
        #[n(3)]
        pub config_version: Option<[u8; 16]>,
        /// Session ID
        #[n(4)]
        pub session_id: [u8; 16],
        /// Proxy name
        #[n(5)]
        pub proxy_name: Option<String>,
        /// Timezone
        #[n(6)]
        pub timezone: Option<String>,
        /// Code completion
        #[n(7)]
        pub gcpp_host: Option<u8>,
    }

    pub mod token_info {
        use super::*;

        #[derive(Clone, PartialEq, Encode, Decode, CborLen)]
        pub struct Token {
            #[n(0)]
            pub provider: String,
            /// User ID (\[u8; 16\])
            #[n(1)]
            pub sub_id: [u8; 16],
            /// Random string (\[u8; 8\])
            #[n(2)]
            pub randomness: [u8; 8],
            /// Generation time (Unix timestamp)
            #[n(3)]
            pub start: i64,
            /// Expiration time (Unix timestamp)
            #[n(4)]
            pub end: i64,
            /// Signature (\[u8; 32\])
            #[n(5)]
            pub signature: [u8; 32],
            /// Whether it's a session token
            #[n(6)]
            pub is_session: bool,
        }
    }

    /// Usage check model rules
    #[derive(Clone, PartialEq, Encode, Decode, CborLen)]
    pub struct UsageCheckModel {
        /// Check type
        #[n(0)]
        pub r#type: usage_check_model::Type,
        /// Model ID list, effective when type is TYPE_CUSTOM
        #[n(1)]
        pub model_ids: Vec<String>,
    }

    pub mod usage_check_model {
        use super::*;

        /// Check type
        #[derive(::serde::Deserialize, Clone, Copy, PartialEq, Encode, Decode, CborLen)]
        #[serde(rename_all = "lowercase")]
        #[cbor(index_only)]
        pub enum Type {
            /// Not specified
            #[n(0)]
            Default = 0,
            /// Disabled
            #[n(1)]
            Disabled = 1,
            /// All
            #[n(2)]
            All = 2,
            /// Custom list
            #[n(3)]
            Custom = 3,
        }
    }
}
