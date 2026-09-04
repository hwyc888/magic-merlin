//! TLS 指纹轮换模块 - 动态改变 TLS 指纹
//!
//! 原理：
//! 1. 每次连接使用不同的 TLS 指纹
//! 2. 防止深信服通过指纹关联多个连接
//! 3. 支持时间/连接数/随机触发轮换
//! 4. 模拟真实用户使用多个浏览器的行为

use std::time::{Duration, Instant};
use rand::Rng;
use super::utls::{TlsFingerprint, get_ja3_fingerprint, ClientHelloBuilder};

/// TLS 指纹轮换配置
#[derive(Clone, Debug)]
pub struct TlsRotationConfig {
    /// 是否启用轮换
    pub enabled: bool,
    /// 轮换策略
    pub strategy: RotationStrategy,
    /// 可用的指纹列表
    pub fingerprints: Vec<TlsFingerprint>,
    /// 时间轮换间隔（秒）
    pub time_interval: u64,
    /// 连接数轮换阈值
    pub connection_threshold: u32,
    /// 是否启用指纹混合
    pub enable_mixing: bool,
}

impl Default for TlsRotationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategy: RotationStrategy::Adaptive,
            fingerprints: vec![
                TlsFingerprint::Chrome120,
                TlsFingerprint::Firefox121,
                TlsFingerprint::Safari17,
                TlsFingerprint::Edge120,
            ],
            time_interval: 300, // 5 分钟
            connection_threshold: 10,
            enable_mixing: true,
        }
    }
}

/// 轮换策略
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RotationStrategy {
    /// 顺序轮换
    Sequential,
    /// 随机轮换
    Random,
    /// 基于时间轮换
    TimeBased,
    /// 基于连接数轮换
    ConnectionBased,
    /// 自适应轮换
    Adaptive,
    /// 加权随机
    WeightedRandom,
}

/// TLS 指纹轮换器
pub struct TlsRotator {
    config: TlsRotationConfig,
    current_index: usize,
    current_fingerprint: TlsFingerprint,
    connection_count: u32,
    last_rotation: Instant,
    rotation_count: u64,
    fingerprint_weights: Vec<u32>,
}

impl TlsRotator {
    pub fn new(config: TlsRotationConfig) -> Self {
        let initial_fp = config.fingerprints.first()
            .copied()
            .unwrap_or(TlsFingerprint::Chrome120);
        
        let weights = vec![100; config.fingerprints.len()];
        
        Self {
            config,
            current_index: 0,
            current_fingerprint: initial_fp,
            connection_count: 0,
            last_rotation: Instant::now(),
            rotation_count: 0,
            fingerprint_weights: weights,
        }
    }

    /// 获取当前指纹
    pub fn current(&self) -> TlsFingerprint {
        self.current_fingerprint
    }

    /// 获取下一个指纹（可能触发轮换）
    pub fn next(&mut self) -> TlsFingerprint {
        if !self.config.enabled || self.config.fingerprints.is_empty() {
            return self.current_fingerprint;
        }

        self.connection_count += 1;
        let should_rotate = self.should_rotate();

        if should_rotate {
            self.rotate();
        }

        self.current_fingerprint
    }

    /// 判断是否应该轮换
    fn should_rotate(&self) -> bool {
        match self.config.strategy {
            RotationStrategy::Sequential => true,
            RotationStrategy::Random => {
                let mut rng = rand::thread_rng();
                rng.gen_bool(0.2) // 20% 概率轮换
            }
            RotationStrategy::TimeBased => {
                self.last_rotation.elapsed() >= Duration::from_secs(self.config.time_interval)
            }
            RotationStrategy::ConnectionBased => {
                self.connection_count >= self.config.connection_threshold
            }
            RotationStrategy::Adaptive => {
                // 综合考虑时间和连接数
                let time_factor = self.last_rotation.elapsed().as_secs() as f64 
                    / self.config.time_interval as f64;
                let conn_factor = self.connection_count as f64 
                    / self.config.connection_threshold as f64;
                
                (time_factor + conn_factor) / 2.0 >= 1.0
            }
            RotationStrategy::WeightedRandom => {
                let mut rng = rand::thread_rng();
                rng.gen_bool(0.1)
            }
        }
    }

    /// 执行轮换
    fn rotate(&mut self) {
        let new_fp = match self.config.strategy {
            RotationStrategy::Sequential => {
                self.current_index = (self.current_index + 1) % self.config.fingerprints.len();
                self.config.fingerprints[self.current_index]
            }
            RotationStrategy::WeightedRandom => {
                self.weighted_random_select()
            }
            _ => {
                let mut rng = rand::thread_rng();
                let idx = rng.gen_range(0..self.config.fingerprints.len());
                self.config.fingerprints[idx]
            }
        };

        self.current_fingerprint = new_fp;
        self.connection_count = 0;
        self.last_rotation = Instant::now();
        self.rotation_count += 1;

        tracing::debug!(
            "TLS 指纹轮换: {:?} -> {:?} (第 {} 次轮换)",
            self.current_fingerprint,
            new_fp,
            self.rotation_count
        );
    }

    /// 加权随机选择
    fn weighted_random_select(&self) -> TlsFingerprint {
        let total: u32 = self.fingerprint_weights.iter().sum();
        let mut rng = rand::thread_rng();
        let mut target = rng.gen_range(0..total);

        for (i, &weight) in self.fingerprint_weights.iter().enumerate() {
            if target < weight {
                return self.config.fingerprints[i];
            }
            target -= weight;
        }

        self.config.fingerprints[0]
    }

    /// 更新指纹权重（基于成功率）
    pub fn update_weight(&mut self, fingerprint: TlsFingerprint, success: bool) {
        if let Some(idx) = self.config.fingerprints.iter().position(|&f| f == fingerprint) {
            if success {
                self.fingerprint_weights[idx] = (self.fingerprint_weights[idx] + 10).min(200);
            } else {
                self.fingerprint_weights[idx] = self.fingerprint_weights[idx].saturating_sub(20).max(10);
            }
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> RotationStats {
        RotationStats {
            current_fingerprint: self.current_fingerprint,
            rotation_count: self.rotation_count,
            connection_count: self.connection_count,
            time_since_rotation: self.last_rotation.elapsed(),
            fingerprint_weights: self.fingerprint_weights.clone(),
        }
    }

    /// 构建当前指纹的 Client Hello
    pub fn build_client_hello(&self, sni: &str) -> Vec<u8> {
        let fp = if self.config.enable_mixing {
            self.mix_fingerprint()
        } else {
            self.current_fingerprint
        };

        ClientHelloBuilder::new(fp, sni.to_string()).build()
    }

    /// 混合指纹（从多个指纹中随机选择扩展）
    fn mix_fingerprint(&self) -> TlsFingerprint {
        // 简化实现：返回当前指纹
        // 完整实现需要混合不同指纹的扩展
        self.current_fingerprint
    }
}

/// 轮换统计信息
#[derive(Clone, Debug)]
pub struct RotationStats {
    pub current_fingerprint: TlsFingerprint,
    pub rotation_count: u64,
    pub connection_count: u32,
    pub time_since_rotation: Duration,
    pub fingerprint_weights: Vec<u32>,
}

/// 指纹混合器
pub struct FingerprintMixer {
    base_fingerprint: TlsFingerprint,
    mix_sources: Vec<TlsFingerprint>,
}

impl FingerprintMixer {
    pub fn new(base: TlsFingerprint) -> Self {
        Self {
            base_fingerprint: base,
            mix_sources: vec![
                TlsFingerprint::Chrome120,
                TlsFingerprint::Firefox121,
                TlsFingerprint::Safari17,
            ],
        }
    }

    /// 创建混合指纹的 Client Hello
    pub fn create_mixed_hello(&self, sni: &str) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        
        // 获取基础指纹
        let base_ja3 = get_ja3_fingerprint(self.base_fingerprint);
        
        // 随机选择一些扩展来自其他指纹
        let mut cipher_suites = base_ja3.cipher_suites.clone();
        let mut extensions = base_ja3.extensions.clone();
        
        // 随机打乱密码套件顺序
        if rng.gen_bool(0.3) {
            use rand::seq::SliceRandom;
            cipher_suites.shuffle(&mut rng);
        }
        
        // 随机添加/移除一些扩展
        if rng.gen_bool(0.2) {
            // 添加 padding 扩展
            if !extensions.contains(&0x0015) {
                extensions.push(0x0015);
            }
        }
        
        // 构建 Client Hello
        ClientHelloBuilder::new(self.base_fingerprint, sni.to_string()).build()
    }
}

/// 指纹伪装检测器
pub struct FingerprintDetector;

impl FingerprintDetector {
    /// 检测 Client Hello 的指纹类型
    pub fn detect(client_hello: &[u8]) -> Option<DetectedFingerprint> {
        if client_hello.len() < 50 {
            return None;
        }

        // 提取特征
        let cipher_suites = Self::extract_cipher_suites(client_hello)?;
        let extensions = Self::extract_extensions(client_hello)?;

        // 匹配已知指纹
        let fingerprint = Self::match_fingerprint(&cipher_suites, &extensions);

        Some(DetectedFingerprint {
            fingerprint,
            cipher_suites,
            extensions,
            confidence: 0.8,
        })
    }

    fn extract_cipher_suites(data: &[u8]) -> Option<Vec<u16>> {
        // 简化实现
        if data.len() < 50 {
            return None;
        }
        
        // 跳过固定头部找到密码套件
        let mut pos = 43; // Random 之后
        if pos >= data.len() { return None; }
        
        let session_id_len = data[pos] as usize;
        pos += 1 + session_id_len;
        
        if pos + 2 > data.len() { return None; }
        let cipher_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        
        if pos + cipher_len > data.len() { return None; }
        
        let mut ciphers = Vec::new();
        for i in (0..cipher_len).step_by(2) {
            if pos + i + 1 < data.len() {
                ciphers.push(u16::from_be_bytes([data[pos + i], data[pos + i + 1]]));
            }
        }
        
        Some(ciphers)
    }

    fn extract_extensions(_data: &[u8]) -> Option<Vec<u16>> {
        // 简化实现：返回空列表
        Some(Vec::new())
    }

    fn match_fingerprint(ciphers: &[u16], _extensions: &[u16]) -> TlsFingerprint {
        // 简单匹配逻辑
        if ciphers.contains(&0x1301) && ciphers.contains(&0xcca9) {
            TlsFingerprint::Chrome120
        } else if ciphers.contains(&0x1303) && ciphers.len() > 10 {
            TlsFingerprint::Firefox121
        } else {
            TlsFingerprint::Chrome120
        }
    }
}

/// 检测到的指纹
#[derive(Clone, Debug)]
pub struct DetectedFingerprint {
    pub fingerprint: TlsFingerprint,
    pub cipher_suites: Vec<u16>,
    pub extensions: Vec<u16>,
    pub confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotator() {
        let config = TlsRotationConfig {
            strategy: RotationStrategy::Sequential,
            ..Default::default()
        };
        let mut rotator = TlsRotator::new(config);

        let fp1 = rotator.next();
        let fp2 = rotator.next();
        
        // 顺序轮换应该返回不同的指纹
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_weighted_selection() {
        let config = TlsRotationConfig {
            strategy: RotationStrategy::WeightedRandom,
            ..Default::default()
        };
        let rotator = TlsRotator::new(config);

        // 多次选择应该有变化
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            seen.insert(format!("{:?}", rotator.weighted_random_select()));
        }
        
        assert!(seen.len() > 1);
    }

    #[test]
    fn test_weight_update() {
        let config = TlsRotationConfig::default();
        let mut rotator = TlsRotator::new(config);

        let initial_weight = rotator.fingerprint_weights[0];
        
        rotator.update_weight(TlsFingerprint::Chrome120, true);
        assert!(rotator.fingerprint_weights[0] > initial_weight);
        
        rotator.update_weight(TlsFingerprint::Chrome120, false);
        // 权重应该下降
    }
}
