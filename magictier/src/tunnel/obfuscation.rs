//! 协议混淆模块 - 用于去除/修改协议特征码
//! 
//! 此模块提供以下功能：
//! 1. 自定义握手魔数
//! 2. 数据包头部混淆
//! 3. 随机填充
//! 4. 特征码替换

use rand::{Rng, SeedableRng};
use std::sync::atomic::{AtomicU64, Ordering};

/// 混淆配置
#[derive(Clone, Debug)]
pub struct ObfuscationConfig {
    /// 自定义握手魔数 (替代 0xd1e1a5e1)
    pub handshake_magic: u32,
    /// 自定义协议版本
    pub protocol_version: u32,
    /// 自定义 STUN Transaction ID 前缀 (替代 0xdeadbeef)
    pub stun_tid_prefix: u32,
    /// 是否启用数据包填充
    pub enable_padding: bool,
    /// 填充大小范围 (min, max)
    pub padding_range: (u16, u16),
    /// 是否启用头部混淆
    pub enable_header_obfuscation: bool,
    /// 头部混淆密钥 (XOR)
    pub header_obfuscation_key: [u8; 16],
}

impl Default for ObfuscationConfig {
    fn default() -> Self {
        // 生成随机默认值，避免使用固定特征
        let mut rng = rand::rngs::StdRng::from_entropy();
        let mut key = [0u8; 16];
        rng.fill(&mut key);
        
        Self {
            // 随机生成魔数，不使用原来的 0xd1e1a5e1
            handshake_magic: rng.gen(),
            protocol_version: rng.gen_range(100..1000),
            // 随机生成 STUN 前缀，不使用 0xdeadbeef
            stun_tid_prefix: rng.gen(),
            enable_padding: true,
            padding_range: (8, 64),
            enable_header_obfuscation: true,
            header_obfuscation_key: key,
        }
    }
}

impl ObfuscationConfig {
    /// 从密钥派生确定性配置
    /// 这样同一网络的节点可以使用相同的混淆参数
    pub fn from_network_secret(secret: &[u8]) -> Self {
        use sha2::{Sha256, Digest};
        
        let mut hasher = Sha256::new();
        // 独有混淆种子 - 全网唯一，基于随机生成
        // 生成时间: 2026-01-07, 随机种子: 0x7f3a9c2e
        hasher.update(b"_xK9mT#vL2pQ8nR$wF5jH@yB3cZ7dA_2026");
        hasher.update(secret);
        let hash = hasher.finalize();
        
        let mut key = [0u8; 16];
        key.copy_from_slice(&hash[..16]);
        
        Self {
            handshake_magic: u32::from_le_bytes([hash[16], hash[17], hash[18], hash[19]]),
            protocol_version: u32::from_le_bytes([hash[20], hash[21], hash[22], hash[23]]) % 900 + 100,
            stun_tid_prefix: u32::from_le_bytes([hash[24], hash[25], hash[26], hash[27]]),
            enable_padding: true,
            padding_range: (8, 64),
            enable_header_obfuscation: true,
            header_obfuscation_key: key,
        }
    }
    
    /// 禁用混淆时使用的固定特征码 - 全网独一无二
    /// 这些值是随机生成的，不与任何已知软件冲突
    pub fn disabled() -> Self {
        Self {
            // 独有握手魔数: 0x7f3a9c2e (随机生成，全网唯一)
            handshake_magic: 0x7f3a9c2e,
            // 独有协议版本: 73 (随机选择)
            protocol_version: 73,
            // 独有 STUN 前缀: 0xe5b8d4f1 (随机生成，全网唯一)
            stun_tid_prefix: 0xe5b8d4f1,
            enable_padding: false,
            padding_range: (0, 0),
            enable_header_obfuscation: false,
            header_obfuscation_key: [0u8; 16],
        }
    }
}

/// 数据包混淆器
pub struct PacketObfuscator {
    config: ObfuscationConfig,
    counter: AtomicU64,
}

impl PacketObfuscator {
    pub fn new(config: ObfuscationConfig) -> Self {
        Self {
            config,
            counter: AtomicU64::new(0),
        }
    }
    
    /// 混淆数据包头部
    pub fn obfuscate_header(&self, header: &mut [u8]) {
        if !self.config.enable_header_obfuscation || header.is_empty() {
            return;
        }
        
        let key = &self.config.header_obfuscation_key;
        for (i, byte) in header.iter_mut().enumerate() {
            *byte ^= key[i % key.len()];
        }
    }
    
    /// 解混淆数据包头部 (XOR 是对称的)
    pub fn deobfuscate_header(&self, header: &mut [u8]) {
        self.obfuscate_header(header);
    }
    
    /// 生成随机填充
    pub fn generate_padding(&self) -> Vec<u8> {
        if !self.config.enable_padding {
            return Vec::new();
        }
        
        let mut rng = rand::thread_rng();
        let (min, max) = self.config.padding_range;
        let len = rng.gen_range(min..=max) as usize;
        
        let mut padding = vec![0u8; len];
        rng.fill(&mut padding[..]);
        padding
    }
    
    /// 获取握手魔数
    pub fn get_handshake_magic(&self) -> u32 {
        self.config.handshake_magic
    }
    
    /// 获取协议版本
    pub fn get_protocol_version(&self) -> u32 {
        self.config.protocol_version
    }
    
    /// 获取 STUN TID 前缀
    pub fn get_stun_tid_prefix(&self) -> u32 {
        self.config.stun_tid_prefix
    }
    
    /// 生成唯一计数器值 (用于 nonce 等)
    pub fn next_counter(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }
}

/// 全局混淆配置
static GLOBAL_OBFUSCATION_CONFIG: std::sync::OnceLock<ObfuscationConfig> = std::sync::OnceLock::new();

/// 初始化全局混淆配置
pub fn init_global_obfuscation(config: ObfuscationConfig) {
    let _ = GLOBAL_OBFUSCATION_CONFIG.set(config);
}

/// 获取全局混淆配置
pub fn get_global_obfuscation() -> &'static ObfuscationConfig {
    GLOBAL_OBFUSCATION_CONFIG.get_or_init(ObfuscationConfig::default)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_header_obfuscation() {
        let config = ObfuscationConfig::default();
        let obfuscator = PacketObfuscator::new(config);
        
        let original = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        let mut data = original.clone();
        
        obfuscator.obfuscate_header(&mut data);
        assert_ne!(data, original);
        
        obfuscator.deobfuscate_header(&mut data);
        assert_eq!(data, original);
    }
    
    #[test]
    fn test_deterministic_config() {
        let secret = b"test_network_secret";
        let config1 = ObfuscationConfig::from_network_secret(secret);
        let config2 = ObfuscationConfig::from_network_secret(secret);
        
        assert_eq!(config1.handshake_magic, config2.handshake_magic);
        assert_eq!(config1.stun_tid_prefix, config2.stun_tid_prefix);
    }
}
