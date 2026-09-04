//! 多路径传输模块 - 2024 流量分散技术
//!
//! 原理：
//! 1. 将数据分散到多个路径/连接传输
//! 2. 每个路径可以使用不同的协议、端口、服务器
//! 3. 深信服难以关联这些分散的流量
//! 4. 提高可靠性和带宽利用率
//!
//! 类似于 MPTCP，但在应用层实现

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use rand::Rng;

/// 多路径配置
#[derive(Clone, Debug)]
pub struct MultiPathConfig {
    /// 是否启用
    pub enabled: bool,
    /// 最小路径数
    pub min_paths: usize,
    /// 最大路径数
    pub max_paths: usize,
    /// 调度算法
    pub scheduler: PathScheduler,
    /// 路径探测间隔
    pub probe_interval: Duration,
    /// 路径超时时间
    pub path_timeout: Duration,
    /// 是否启用路径聚合
    pub aggregation: bool,
}

impl Default for MultiPathConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_paths: 2,
            max_paths: 8,
            scheduler: PathScheduler::WeightedRoundRobin,
            probe_interval: Duration::from_secs(10),
            path_timeout: Duration::from_secs(30),
            aggregation: true,
        }
    }
}

/// 路径调度算法
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathScheduler {
    /// 轮询
    RoundRobin,
    /// 加权轮询
    WeightedRoundRobin,
    /// 最低延迟
    LowestLatency,
    /// 最高带宽
    HighestBandwidth,
    /// 随机
    Random,
    /// 冗余（所有路径都发）
    Redundant,
}

/// 路径状态
#[derive(Clone, Debug)]
pub struct PathState {
    /// 路径 ID
    pub id: u32,
    /// 路径地址
    pub addr: String,
    /// 协议类型
    pub protocol: String,
    /// 是否活跃
    pub active: bool,
    /// RTT
    pub rtt: Duration,
    /// 带宽估计 (bytes/s)
    pub bandwidth: u64,
    /// 丢包率
    pub loss_rate: f64,
    /// 权重
    pub weight: u32,
    /// 已发送字节
    pub bytes_sent: u64,
    /// 已接收字节
    pub bytes_received: u64,
    /// 上次活跃时间
    pub last_active: Instant,
}

impl PathState {
    pub fn new(id: u32, addr: String, protocol: String) -> Self {
        Self {
            id,
            addr,
            protocol,
            active: true,
            rtt: Duration::from_millis(100),
            bandwidth: 1_000_000, // 1 MB/s 默认
            loss_rate: 0.0,
            weight: 100,
            bytes_sent: 0,
            bytes_received: 0,
            last_active: Instant::now(),
        }
    }

    /// 计算路径分数（用于调度）
    pub fn score(&self) -> f64 {
        if !self.active {
            return 0.0;
        }
        
        let rtt_score = 1.0 / (self.rtt.as_millis() as f64 + 1.0);
        let bw_score = self.bandwidth as f64 / 1_000_000.0;
        let loss_score = 1.0 - self.loss_rate;
        
        (rtt_score * 0.3 + bw_score * 0.5 + loss_score * 0.2) * self.weight as f64
    }
}

/// 多路径管理器
pub struct MultiPathManager {
    config: MultiPathConfig,
    paths: Arc<RwLock<HashMap<u32, PathState>>>,
    next_path_id: Arc<Mutex<u32>>,
    current_path_index: Arc<Mutex<usize>>,
    sequence_number: Arc<Mutex<u64>>,
}

impl MultiPathManager {
    pub fn new(config: MultiPathConfig) -> Self {
        Self {
            config,
            paths: Arc::new(RwLock::new(HashMap::new())),
            next_path_id: Arc::new(Mutex::new(0)),
            current_path_index: Arc::new(Mutex::new(0)),
            sequence_number: Arc::new(Mutex::new(0)),
        }
    }

    /// 添加路径
    pub async fn add_path(&self, addr: String, protocol: String) -> u32 {
        let mut next_id = self.next_path_id.lock().await;
        let id = *next_id;
        *next_id += 1;
        
        let path = PathState::new(id, addr, protocol);
        self.paths.write().await.insert(id, path);
        
        id
    }

    /// 移除路径
    pub async fn remove_path(&self, id: u32) {
        self.paths.write().await.remove(&id);
    }

    /// 获取活跃路径数
    pub async fn active_path_count(&self) -> usize {
        self.paths.read().await
            .values()
            .filter(|p| p.active)
            .count()
    }

    /// 选择下一个路径
    pub async fn select_path(&self) -> Option<u32> {
        let paths = self.paths.read().await;
        let active_paths: Vec<_> = paths.values()
            .filter(|p| p.active)
            .collect();
        
        if active_paths.is_empty() {
            return None;
        }
        
        match self.config.scheduler {
            PathScheduler::RoundRobin => {
                let mut index = self.current_path_index.lock().await;
                *index = (*index + 1) % active_paths.len();
                Some(active_paths[*index].id)
            }
            PathScheduler::WeightedRoundRobin => {
                self.weighted_round_robin(&active_paths).await
            }
            PathScheduler::LowestLatency => {
                active_paths.iter()
                    .min_by_key(|p| p.rtt)
                    .map(|p| p.id)
            }
            PathScheduler::HighestBandwidth => {
                active_paths.iter()
                    .max_by_key(|p| p.bandwidth)
                    .map(|p| p.id)
            }
            PathScheduler::Random => {
                let idx = rand::thread_rng().gen_range(0..active_paths.len());
                Some(active_paths[idx].id)
            }
            PathScheduler::Redundant => {
                // 返回第一个，实际发送时会发送到所有路径
                active_paths.first().map(|p| p.id)
            }
        }
    }

    /// 加权轮询选择
    async fn weighted_round_robin(&self, paths: &[&PathState]) -> Option<u32> {
        let total_weight: u32 = paths.iter().map(|p| p.weight).sum();
        if total_weight == 0 {
            return paths.first().map(|p| p.id);
        }
        
        let mut rng = rand::thread_rng();
        let mut target = rng.gen_range(0..total_weight);
        
        for path in paths {
            if target < path.weight {
                return Some(path.id);
            }
            target -= path.weight;
        }
        
        paths.first().map(|p| p.id)
    }

    /// 获取所有活跃路径（用于冗余发送）
    pub async fn get_all_active_paths(&self) -> Vec<u32> {
        self.paths.read().await
            .values()
            .filter(|p| p.active)
            .map(|p| p.id)
            .collect()
    }

    /// 更新路径状态
    pub async fn update_path_stats(&self, id: u32, rtt: Duration, bytes: u64, lost: bool) {
        let mut paths = self.paths.write().await;
        if let Some(path) = paths.get_mut(&id) {
            path.rtt = rtt;
            path.bytes_sent += bytes;
            path.last_active = Instant::now();
            
            if lost {
                path.loss_rate = (path.loss_rate * 0.9) + 0.1;
            } else {
                path.loss_rate *= 0.99;
            }
            
            // 更新权重
            path.weight = (path.score() * 100.0) as u32;
        }
    }

    /// 标记路径为不活跃
    pub async fn mark_path_inactive(&self, id: u32) {
        let mut paths = self.paths.write().await;
        if let Some(path) = paths.get_mut(&id) {
            path.active = false;
        }
    }

    /// 获取下一个序列号
    pub async fn next_sequence(&self) -> u64 {
        let mut seq = self.sequence_number.lock().await;
        let current = *seq;
        *seq += 1;
        current
    }

    /// 获取路径统计
    pub async fn get_stats(&self) -> Vec<PathState> {
        self.paths.read().await.values().cloned().collect()
    }
}

/// 多路径数据包头
#[derive(Clone, Debug)]
pub struct MultiPathHeader {
    /// 序列号
    pub sequence: u64,
    /// 路径 ID
    pub path_id: u32,
    /// 是否是冗余包
    pub redundant: bool,
    /// 时间戳
    pub timestamp: u64,
}

impl MultiPathHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(21);
        buf.extend_from_slice(&self.sequence.to_be_bytes());
        buf.extend_from_slice(&self.path_id.to_be_bytes());
        buf.push(if self.redundant { 1 } else { 0 });
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf
    }

    pub fn decode(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 21 {
            return Err("数据太短");
        }
        
        Ok(Self {
            sequence: u64::from_be_bytes(data[0..8].try_into().unwrap()),
            path_id: u32::from_be_bytes(data[8..12].try_into().unwrap()),
            redundant: data[12] != 0,
            timestamp: u64::from_be_bytes(data[13..21].try_into().unwrap()),
        })
    }
}

/// 多路径重组器
pub struct MultiPathReassembler {
    /// 已接收的序列号
    received: HashMap<u64, Vec<u8>>,
    /// 下一个期望的序列号
    next_expected: u64,
    /// 输出通道
    output: mpsc::Sender<Vec<u8>>,
}

impl MultiPathReassembler {
    pub fn new(output: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            received: HashMap::new(),
            next_expected: 0,
            output,
        }
    }

    /// 处理接收到的数据包
    pub async fn process(&mut self, header: MultiPathHeader, data: Vec<u8>) {
        // 忽略重复的冗余包
        if header.redundant && self.received.contains_key(&header.sequence) {
            return;
        }
        
        self.received.insert(header.sequence, data);
        
        // 尝试按序输出
        while let Some(data) = self.received.remove(&self.next_expected) {
            let _ = self.output.send(data).await;
            self.next_expected += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multi_path_manager() {
        let config = MultiPathConfig::default();
        let manager = MultiPathManager::new(config);
        
        // 添加路径
        let id1 = manager.add_path("1.1.1.1:443".to_string(), "tcp".to_string()).await;
        let id2 = manager.add_path("2.2.2.2:443".to_string(), "udp".to_string()).await;
        
        assert_eq!(manager.active_path_count().await, 2);
        
        // 选择路径
        let selected = manager.select_path().await;
        assert!(selected.is_some());
        
        // 移除路径
        manager.remove_path(id1).await;
        assert_eq!(manager.active_path_count().await, 1);
        
        let _ = id2;
    }

    #[test]
    fn test_header_encode_decode() {
        let header = MultiPathHeader {
            sequence: 12345,
            path_id: 1,
            redundant: false,
            timestamp: 1234567890,
        };
        
        let encoded = header.encode();
        let decoded = MultiPathHeader::decode(&encoded).unwrap();
        
        assert_eq!(decoded.sequence, header.sequence);
        assert_eq!(decoded.path_id, header.path_id);
        assert_eq!(decoded.redundant, header.redundant);
    }

    #[test]
    fn test_path_score() {
        let mut path = PathState::new(0, "test".to_string(), "tcp".to_string());
        
        let score1 = path.score();
        assert!(score1 > 0.0);
        
        // 更好的路径应该有更高的分数
        path.rtt = Duration::from_millis(10);
        path.bandwidth = 10_000_000;
        let score2 = path.score();
        
        assert!(score2 > score1);
    }
}
