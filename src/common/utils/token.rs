use crate::{app::constant::HEADER_B64, common::model::token::TokenPayload};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Local, TimeZone as _};

// 解析token
// pub fn parse_token(token_part: &str) -> String {
//     // 查找Last一个:Or%3A的位置
//     let colon_pos = token_part.rfind(':');
//     let encoded_colon_pos = token_part.rfind("%3A");

//     match (colon_pos, encoded_colon_pos) {
//         (None, None) => token_part.to_string(),
//         (Some(pos1), None) => token_part[(pos1 + 1)..].to_string(),
//         (None, Some(pos2)) => token_part[(pos2 + 3)..].to_string(),
//         (Some(pos1), Some(pos2)) => {
//             // 取较大的位置作ToSeparate点
//             let pos = pos1.max(pos2);
//             let start = if pos == pos2 { pos + 3 } else { pos + 1 };
//             token_part[start..].to_string()
//         }
//     }
// }

// Token 加载函数，Support从字符串Content加载
// pub fn load_tokens_from_content(content: &str) -> Vec<TokenInfo> {
//     let token_map: std::collections::HashMap<String, String> = content
//         .lines()
//         .filter_map(|line| {
//             let line = line.trim();
//             if line.is_empty() || line.starts_with('#') {
//                 return None;
//             }

//             let parts: Vec<&str> = line.split(COMMA).collect();
//             match parts[..] {
//                 [token_part, checksum] => {
//                     let token = parse_token(token_part);
//                     Some((token, generate_checksum_with_repair(checksum)))
//                 }
//                 _ => {
//                     eprintln!("Warning: 忽略无效的token-list行: {line}");
//                     None
//                 }
//             }
//         })
//         .collect();

//     token_map
//         .into_iter()
//         .map(|(token, checksum)| TokenInfo {
//             token,
//             checksum,
//             profile: None,
//             tags: None,
//         })
//         .collect()
// }

#[rustfmt::skip]
pub fn validate_token(token: &str) -> bool {
    // Check token Formatand分割
    let Some(parts) = token.strip_prefix(HEADER_B64) else { return false };
    let Some((payload, signature)) = parts.split_once('.') else { return false };

    // Decode signature and payload
    let Ok(signature) = URL_SAFE_NO_PAD.decode(signature) else { return false };
    if signature.len() != 32 { return false };
    let Ok(payload) = URL_SAFE_NO_PAD.decode(payload) else { return false };

    // ConvertTo字符串并解析
    let Ok(payload_str) = String::from_utf8(payload) else { return false };
    let Ok(payload) = serde_json::from_str::<TokenPayload>(&payload_str) else { return false };

    // 验证时间
    if payload.time.0 > chrono::Utc::now().timestamp() {
        return false;
    }

    // 验证 randomness
    let bytes = payload.randomness.as_bytes();
    if bytes.len() != 18 {
        return false;
    }

    // 验证字符Format
    for (i, &b) in bytes.iter().enumerate() {
        let valid = match i {
            0..=7 | 9..=12 | 14..=17 => b.is_ascii_hexdigit(),
            8 | 13 => b == b'-',
            _ => unreachable!(),
        };
        if !valid {
            return false;
        }
    }

    true
}

/// 从 JWT token 中提取用户 ID
pub fn extract_user_id(token: &str) -> Option<String> {
    let mut parts = [None; 3];
    let mut count = 0;

    for part in token.split('.') {
        if count >= 3 {
            return None;
        }
        parts[count] = Some(part);
        count += 1;
    }
    if count != 3 {
        return None;
    }

    let payload = URL_SAFE_NO_PAD
        .decode(unsafe { *parts.get_unchecked(1) }?)
        .ok()?;

    let payload_str = String::from_utf8_lossy(&payload);
    let payload: TokenPayload = serde_json::from_str(&payload_str).ok()?;

    payload.sub.split('|').nth(1).map(|id| id.to_string())
}

#[derive(serde::Serialize)]
pub struct JwtTime {
    pub iat: DateTime<Local>,
    pub exp: DateTime<Local>,
}

// 从 JWT token 中提取 time Field
pub fn extract_time(token: &str) -> Option<JwtTime> {
    // JWT token 由3部分组成，用 . Separate
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // Decode payload (第二部分)
    let payload = match URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(decoded) => decoded,
        Err(_) => return None,
    };

    drop(parts);

    // 将 payload ConvertTo字符串
    let payload_str = match String::from_utf8(payload) {
        Ok(s) => s,
        Err(_) => return None,
    };

    // 解析To TokenPayload
    let payload: TokenPayload = match serde_json::from_str(&payload_str) {
        Ok(p) => p,
        Err(_) => return None,
    };

    let iat = Local.timestamp_opt(payload.time.0, 0).single()?;
    let exp = Local.timestamp_opt(payload.exp, 0).single()?;

    Some(JwtTime { iat, exp })
}
