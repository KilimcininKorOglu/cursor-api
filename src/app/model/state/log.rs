use crate::app::{
    lazy::LOGS_FILE_PATH,
    model::{ExtToken, ExtTokenHelper, RequestLog, TokenKey, log::RequestLogHelper},
};
use alloc::collections::VecDeque;
use memmap2::{MmapMut, MmapOptions};
use rkyv::{
    Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize, rancor::Error as RkyvError,
};
use tokio::fs::OpenOptions;

type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;

/// Request日志Limit枚举
#[derive(Debug, Clone, Copy)]
pub enum RequestLogsLimit {
    /// Disabled日志记录
    Disabled,
    /// 无Limit日志记录
    Unlimited,
    /// HaveLimit的日志记录，参数To最大日志数Amount
    Limited(usize),
}

impl RequestLogsLimit {
    /// 从usize创建RequestLogsLimit
    #[inline]
    pub fn from_usize(limit: usize) -> Self {
        const MAX_LIMIT: usize = 1000000;
        match limit {
            0 => Self::Disabled,
            n if n >= MAX_LIMIT => Self::Unlimited,
            n => Self::Limited(n),
        }
    }

    /// CheckWhetherNeed保存日志
    #[inline(always)]
    pub fn should_log(&self) -> bool { !matches!(self, Self::Disabled) }

    /// Get日志Limit
    #[inline(always)]
    pub fn get_limit(&self) -> Option<usize> {
        match self {
            Self::Disabled => Some(0),
            Self::Unlimited => None,
            Self::Limited(limit) => Some(*limit),
        }
    }
}

/// 用于rkyv序列化的辅助结构
#[derive(Archive, RkyvDeserialize, RkyvSerialize)]
struct LogManagerHelper {
    logs: Vec<RequestLogHelper>,
    tokens: HashMap<TokenKey, ExtTokenHelper>,
}

/// Log管理器，负责Handle日志andtoken的集中管理
pub struct LogManager {
    logs: VecDeque<RequestLog>,
    tokens: HashMap<TokenKey, ExtToken>,
    token_ref_counts: HashMap<TokenKey, usize>, // token引用计数
    logs_limit: RequestLogsLimit,
}

impl LogManager {
    /// CreateNew日志管理器
    #[inline]
    pub fn new(logs_limit: RequestLogsLimit) -> Self {
        Self {
            logs: match logs_limit {
                RequestLogsLimit::Disabled => VecDeque::new(),
                RequestLogsLimit::Limited(limit) if limit < 128 => VecDeque::with_capacity(limit),
                _ => VecDeque::with_capacity(32),
            },
            tokens: HashMap::default(),
            token_ref_counts: HashMap::default(),
            logs_limit,
        }
    }

    /// 从存储中加载日志
    #[inline(never)]
    pub async fn load() -> Result<Self, Box<dyn core::error::Error + Send + Sync + 'static>> {
        let logs_limit = RequestLogsLimit::from_usize(crate::common::utils::parse_from_env(
            "REQUEST_LOGS_LIMIT",
            100usize,
        ));

        // IfDisabled日志，则返回Empty管理器
        if !logs_limit.should_log() {
            return Ok(Self::new(logs_limit));
        }

        let (logs, tokens) = {
            let file = match OpenOptions::new().read(true).open(&*LOGS_FILE_PATH).await {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Self::new(logs_limit));
                }
                Err(e) => return Err(Box::new(e)),
            };

            if file.metadata().await?.len() > usize::MAX as u64 {
                return Err("日志文件过大".into());
            }

            let mmap = unsafe { MmapOptions::new().map(&file)? };
            let helper =
                unsafe { ::rkyv::from_bytes_unchecked::<LogManagerHelper, RkyvError>(&mmap) }?;

            let logs = helper.logs.into_iter().map(RequestLogHelper::into_request_log).collect();

            let tokens = helper.tokens.into_iter().map(|(k, v)| (k, v.extract())).collect();

            (logs, tokens)
        };
        let mut manager = Self { logs, tokens, token_ref_counts: HashMap::default(), logs_limit };

        // 重建token引用计数
        manager.rebuild_token_ref_counts();

        Ok(manager)
    }

    /// 重建token引用计数
    #[inline(never)]
    fn rebuild_token_ref_counts(&mut self) {
        self.token_ref_counts.clear();

        // Statistics每个token被多少个日志引用
        for log in &self.logs {
            let token_key = log.token_key();
            *self.token_ref_counts.entry(token_key).or_insert(0) += 1;
        }

        // 移除没Have被引用的tokens
        self.tokens.retain(|key, _| self.token_ref_counts.contains_key(key));
    }

    /// 增加token引用计数
    #[inline]
    fn increment_token_ref(&mut self, token_key: TokenKey) {
        *self.token_ref_counts.entry(token_key).or_insert(0) += 1;
    }

    /// 减少token引用计数，If计数To0则清理token
    #[inline]
    fn decrement_token_ref(&mut self, token_key: TokenKey) {
        if let Some(count) = self.token_ref_counts.get_mut(&token_key) {
            *count -= 1;
            if *count == 0 {
                self.token_ref_counts.remove(&token_key);
                self.tokens.remove(&token_key);
            }
        }
    }

    /// 内部方法：添加Or更新token（仅在Need时调用）
    #[inline]
    fn insert_token(&mut self, key: TokenKey, token: ExtToken) { self.tokens.insert(key, token); }

    /// 公开方法：添加日志时同时更新Relatedtoken
    #[inline(never)]
    pub fn push_log_with_token(&mut self, log: RequestLog, ext_token: ExtToken) {
        // IfDisabled日志，则直接返回
        if !self.logs_limit.should_log() {
            return;
        }

        let log_token_key = log.token_key();

        // 根据Limit策略管理日志队列
        if let Some(limit) = self.logs_limit.get_limit() {
            while self.logs.len() >= limit {
                if let Some(removed_log) = self.logs.pop_front() {
                    // 减少被移除日志的token引用计数
                    let removed_token_key = removed_log.token_key();
                    self.decrement_token_ref(removed_token_key);
                }
            }
        }

        // Add新token（If提供且不存在If）
        // debug_assert_eq!(token_key, log_token_key, "token key 与日志中的不匹配");
        self.insert_token(log_token_key, ext_token);

        // 增加新日志的token引用计数
        self.increment_token_ref(log_token_key);

        // Add日志
        self.logs.push_back(log);
    }

    /// 保存数据到文件
    #[inline(never)]
    pub async fn save(&self) -> Result<(), Box<dyn core::error::Error + Send + Sync + 'static>> {
        // IfDisabled日志，则跳过保存
        if !self.logs_limit.should_log() {
            return Ok(());
        }

        let helper = LogManagerHelper {
            logs: self.logs.iter().map(RequestLogHelper::from).collect(),
            tokens: self.tokens.iter().map(|(k, v)| (*k, ExtTokenHelper::new(v))).collect(),
        };

        let bytes = rkyv::to_bytes::<RkyvError>(&helper)?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*LOGS_FILE_PATH)
            .await?;

        if bytes.len() > usize::MAX >> 1 {
            return Err("日志数据过大".into());
        }

        file.set_len(bytes.len() as u64).await?;
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        mmap.copy_from_slice(&bytes);
        mmap.flush()?;

        Ok(())
    }

    /// Get日志的只读引用
    #[inline]
    pub fn logs(&self) -> &VecDeque<RequestLog> { &self.logs }

    // /// Get日志的可变引用
    // #[inline]
    // pub fn logs_mut(&mut self) -> &mut VecDeque<RequestLog> {
    //     &mut self.logs
    // }

    /// Gettoken的只读引用
    #[inline]
    pub fn tokens(&self) -> &HashMap<TokenKey, ExtToken> { &self.tokens }

    // /// 兼容性方法：添加新日志（不推荐，建议Usepush_log_with_token）
    // #[inline]
    // pub fn push_log(&mut self, log: RequestLog) {
    //     self.push_log_with_token(log, None);
    // }

    /// Gettoken
    #[inline]
    pub fn get_token(&self, key: &TokenKey) -> Option<&ExtToken> { self.tokens.get(key) }

    /// Get下一个日志ID
    #[inline]
    pub fn next_log_id(&self) -> u64 { self.logs.back().map_or(1, |log| log.id + 1) }

    /// 查找指定ID的日志并修改
    #[inline]
    pub fn update_log<F>(&mut self, id: u64, f: F)
    where F: FnOnce(&mut RequestLog) {
        if let Some(log) = self.logs.iter_mut().rev().find(|log| log.id == id) {
            f(log)
        }
    }

    // /// 移除指定ID的日志
    // #[inline]
    // pub fn remove_log(&mut self, id: u64) -> bool {
    //     if let Some(pos) = self.logs.iter().position(|log| log.id == id) {
    //         if let Some(removed_log) = self.logs.remove(pos) {
    //             // 减少被移除日志的token引用计数
    //             let token_key = removed_log.token_key();
    //             self.decrement_token_ref(token_key);
    //             return true;
    //         }
    //     }
    //     false
    // }

    /// CheckWhether启用日志
    #[inline]
    pub fn is_enabled(&self) -> bool { self.logs_limit.should_log() }

    /// GetError日志数Amount
    #[inline]
    pub fn error_count(&self) -> u64 {
        self.logs.iter().filter(|log| log.status as u8 != 1).count() as u64
    }

    /// Get日志总数
    #[inline]
    pub fn total_count(&self) -> u64 { self.logs.len() as u64 }

    // /// Gettoken总数
    // #[inline]
    // pub fn token_count(&self) -> u64 {
    //     self.tokens.len() as u64
    // }

    // /// Gettoken引用计数Statistics
    // #[inline]
    // pub fn token_ref_stats(&self) -> Vec<(TokenKey, usize)> {
    //     self.token_ref_counts
    //         .iter()
    //         .map(|(&k, &v)| (k, v))
    //         .collect()
    // }

    // /// 清Empty所Have日志andtoken
    // #[inline]
    // pub fn clear(&mut self) {
    //     self.logs.clear();
    //     self.tokens.clear();
    //     self.token_ref_counts.clear();
    // }

    // /// 清Empty日志，自动清理未Use的token
    // #[inline]
    // pub fn clear_logs(&mut self) {
    //     self.logs.clear();
    //     self.tokens.clear();
    //     self.token_ref_counts.clear();
    // }

    // /// 手动清理未Use的token
    // #[inline(never)]
    // pub fn cleanup_unused_tokens(&mut self) {
    //     self.rebuild_token_ref_counts();
    // }

    /// 根据ID查找日志
    #[inline]
    pub fn find_log(&self, id: u64) -> Option<&RequestLog> {
        self.logs.iter().rev().find(|log| log.id == id)
    }

    // /// 根据ID查找可变日志
    // #[inline]
    // pub fn find_log_mut(&mut self, id: u64) -> Option<&mut RequestLog> {
    //     self.logs.iter_mut().rev().find(|log| log.id == id)
    // }

    // /// 遍历日志and对应token的迭代器
    // #[inline]
    // pub fn logs_with_tokens(&self) -> impl Iterator<Item = (&RequestLog, &ExtToken)> {
    //     self.logs.iter().filter_map(|log| {
    //         self.get_token(&log.token_info.key)
    //             .map(|token| (log, token))
    //     })
    // }
}
