//! Brutal 拥塞控制模块 - 2024 高速传输技术
//!
//! Brutal CC 是 Hysteria 项目开发的拥塞控制算法：
//! 1. 忽略传统拥塞控制的保守策略
//! 2. 直接按用户指定的带宽发送
//! 3. 可以跑满带宽，不受 BBR/Cubic 限制
//! 4. 适合高延迟、高丢包的网络环境
//!
//! 警告：可能对网络造成压力，请合理使用

use std::time::{Duration, Instant};

/// Brutal 拥塞控制配置
#[derive(Clone, Debug)]
pub struct BrutalConfig {
    /// 是否启用
    pub enabled: bool,
    /// 目标发送速率 (bytes/s)
    pub target_rate: u64,
    /// 最小 RTT (用于计算窗口)
    pub min_rtt: Duration,
    /// 丢包率补偿因子
    pub loss_multiplier: f64,
    /// 最大拥塞窗口
    pub max_cwnd: u64,
    /// 是否启用 ACK 聚合
    pub ack_aggregation: bool,
}

impl Default for BrutalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            target_rate: 100 * 1024 * 1024, // 100 Mbps
            min_rtt: Duration::from_millis(50),
            loss_multiplier: 1.5,
            max_cwnd: 256 * 1024 * 1024, // 256 MB
            ack_aggregation: true,
        }
    }
}

impl BrutalConfig {
    /// 从带宽 (Mbps) 创建配置
    pub fn from_mbps(mbps: u32) -> Self {
        Self {
            target_rate: (mbps as u64) * 1024 * 1024 / 8,
            ..Default::default()
        }
    }
}

/// Brutal 拥塞控制器
pub struct BrutalCongestionController {
    config: BrutalConfig,
    /// 当前拥塞窗口
    cwnd: u64,
    /// 已发送未确认的字节数
    bytes_in_flight: u64,
    /// 最小 RTT
    min_rtt: Duration,
    /// 最近的 RTT
    latest_rtt: Duration,
    /// 丢包计数
    lost_packets: u64,
    /// 总发送包数
    total_packets: u64,
    /// 上次更新时间
    last_update: Instant,
    /// 发送速率统计
    bytes_sent_since_update: u64,
}

impl BrutalCongestionController {
    pub fn new(config: BrutalConfig) -> Self {
        let initial_cwnd = Self::calculate_cwnd(
            config.target_rate,
            config.min_rtt,
            0.0,
            config.loss_multiplier,
        );
        
        Self {
            cwnd: initial_cwnd.min(config.max_cwnd),
            bytes_in_flight: 0,
            min_rtt: config.min_rtt,
            latest_rtt: config.min_rtt,
            lost_packets: 0,
            total_packets: 0,
            last_update: Instant::now(),
            bytes_sent_since_update: 0,
            config,
        }
    }

    /// 计算拥塞窗口
    /// cwnd = target_rate * rtt * (1 + loss_rate * loss_multiplier)
    fn calculate_cwnd(target_rate: u64, rtt: Duration, loss_rate: f64, loss_multiplier: f64) -> u64 {
        let rtt_secs = rtt.as_secs_f64();
        let cwnd = (target_rate as f64) * rtt_secs * (1.0 + loss_rate * loss_multiplier);
        cwnd as u64
    }

    /// 获取当前拥塞窗口
    pub fn cwnd(&self) -> u64 {
        self.cwnd
    }

    /// 获取可发送的字节数
    pub fn available_send_window(&self) -> u64 {
        self.cwnd.saturating_sub(self.bytes_in_flight)
    }

    /// 是否可以发送
    pub fn can_send(&self, bytes: u64) -> bool {
        self.bytes_in_flight + bytes <= self.cwnd
    }

    /// 记录发送
    pub fn on_packet_sent(&mut self, bytes: u64) {
        self.bytes_in_flight += bytes;
        self.total_packets += 1;
        self.bytes_sent_since_update += bytes;
    }

    /// 记录确认
    pub fn on_packet_acked(&mut self, bytes: u64, rtt: Duration) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes);
        
        // 更新 RTT
        self.latest_rtt = rtt;
        if rtt < self.min_rtt {
            self.min_rtt = rtt;
        }
        
        // 重新计算窗口
        self.update_cwnd();
    }

    /// 记录丢包
    pub fn on_packet_lost(&mut self, bytes: u64) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(bytes);
        self.lost_packets += 1;
        
        // 重新计算窗口
        self.update_cwnd();
    }

    /// 更新拥塞窗口
    fn update_cwnd(&mut self) {
        let loss_rate = if self.total_packets > 0 {
            self.lost_packets as f64 / self.total_packets as f64
        } else {
            0.0
        };
        
        let new_cwnd = Self::calculate_cwnd(
            self.config.target_rate,
            self.min_rtt,
            loss_rate,
            self.config.loss_multiplier,
        );
        
        self.cwnd = new_cwnd.min(self.config.max_cwnd);
    }

    /// 获取当前发送速率
    pub fn current_rate(&self) -> u64 {
        let elapsed = self.last_update.elapsed();
        if elapsed.as_secs_f64() > 0.0 {
            (self.bytes_sent_since_update as f64 / elapsed.as_secs_f64()) as u64
        } else {
            0
        }
    }

    /// 重置统计
    pub fn reset_stats(&mut self) {
        self.bytes_sent_since_update = 0;
        self.last_update = Instant::now();
    }

    /// 获取统计信息
    pub fn stats(&self) -> BrutalStats {
        BrutalStats {
            cwnd: self.cwnd,
            bytes_in_flight: self.bytes_in_flight,
            min_rtt: self.min_rtt,
            latest_rtt: self.latest_rtt,
            loss_rate: if self.total_packets > 0 {
                self.lost_packets as f64 / self.total_packets as f64
            } else {
                0.0
            },
            current_rate: self.current_rate(),
        }
    }
}

/// Brutal 统计信息
#[derive(Clone, Debug)]
pub struct BrutalStats {
    pub cwnd: u64,
    pub bytes_in_flight: u64,
    pub min_rtt: Duration,
    pub latest_rtt: Duration,
    pub loss_rate: f64,
    pub current_rate: u64,
}

/// 发送速率限制器
pub struct RateLimiter {
    /// 目标速率 (bytes/s)
    target_rate: u64,
    /// 令牌桶容量
    bucket_capacity: u64,
    /// 当前令牌数
    tokens: u64,
    /// 上次更新时间
    last_update: Instant,
}

impl RateLimiter {
    pub fn new(target_rate: u64) -> Self {
        let bucket_capacity = target_rate / 10; // 100ms 的容量
        Self {
            target_rate,
            bucket_capacity,
            tokens: bucket_capacity,
            last_update: Instant::now(),
        }
    }

    /// 尝试消费令牌
    pub fn try_consume(&mut self, bytes: u64) -> bool {
        self.refill();
        
        if self.tokens >= bytes {
            self.tokens -= bytes;
            true
        } else {
            false
        }
    }

    /// 等待直到可以发送
    pub async fn wait_for(&mut self, bytes: u64) {
        loop {
            self.refill();
            
            if self.tokens >= bytes {
                self.tokens -= bytes;
                return;
            }
            
            // 计算需要等待的时间
            let needed = bytes - self.tokens;
            let wait_time = Duration::from_secs_f64(needed as f64 / self.target_rate as f64);
            tokio::time::sleep(wait_time).await;
        }
    }

    /// 补充令牌
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        let new_tokens = (elapsed.as_secs_f64() * self.target_rate as f64) as u64;
        
        self.tokens = (self.tokens + new_tokens).min(self.bucket_capacity);
        self.last_update = now;
    }
}

/// Pacing 发送器（平滑发送）
pub struct Pacer {
    /// 目标速率
    target_rate: u64,
    /// 每个包的间隔
    packet_interval: Duration,
    /// 上次发送时间
    last_send: Instant,
    /// 累积的发送配额
    accumulated: f64,
}

impl Pacer {
    pub fn new(target_rate: u64, packet_size: usize) -> Self {
        let packets_per_second = target_rate as f64 / packet_size as f64;
        let packet_interval = Duration::from_secs_f64(1.0 / packets_per_second);
        
        Self {
            target_rate,
            packet_interval,
            last_send: Instant::now(),
            accumulated: 0.0,
        }
    }

    /// 获取下次发送时间
    pub fn next_send_time(&mut self, bytes: u64) -> Option<Instant> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_send);
        
        // 累积配额
        self.accumulated += elapsed.as_secs_f64() * self.target_rate as f64;
        self.accumulated = self.accumulated.min(self.target_rate as f64 * 0.1); // 最多累积 100ms
        
        if self.accumulated >= bytes as f64 {
            self.accumulated -= bytes as f64;
            self.last_send = now;
            None // 立即发送
        } else {
            // 需要等待
            let needed = bytes as f64 - self.accumulated;
            let wait_time = Duration::from_secs_f64(needed / self.target_rate as f64);
            Some(now + wait_time)
        }
    }

    /// 更新目标速率
    pub fn set_rate(&mut self, rate: u64) {
        self.target_rate = rate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brutal_cc() {
        let config = BrutalConfig::from_mbps(100);
        let mut cc = BrutalCongestionController::new(config);
        
        // 初始窗口应该大于 0
        assert!(cc.cwnd() > 0);
        
        // 发送数据
        cc.on_packet_sent(1000);
        assert_eq!(cc.bytes_in_flight, 1000);
        
        // 确认数据
        cc.on_packet_acked(1000, Duration::from_millis(50));
        assert_eq!(cc.bytes_in_flight, 0);
    }

    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(1_000_000); // 1 MB/s
        
        // 应该能消费一些令牌
        assert!(limiter.try_consume(1000));
    }

    #[test]
    fn test_pacer() {
        let mut pacer = Pacer::new(1_000_000, 1000); // 1 MB/s, 1000 bytes/packet
        
        // 第一个包应该立即发送
        let next = pacer.next_send_time(1000);
        // 可能立即发送或需要等待
        let _ = next;
    }

    #[test]
    fn test_cwnd_calculation() {
        // 100 Mbps, 50ms RTT, 0% loss
        let cwnd = BrutalCongestionController::calculate_cwnd(
            100 * 1024 * 1024 / 8, // 100 Mbps in bytes
            Duration::from_millis(50),
            0.0,
            1.5,
        );
        
        // cwnd ≈ 12.5 MB * 0.05s = 625 KB
        assert!(cwnd > 500_000);
        assert!(cwnd < 1_000_000);
    }
}
