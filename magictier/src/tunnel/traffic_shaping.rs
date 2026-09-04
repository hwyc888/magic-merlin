//! 流量整形模块 - 对抗深信服流量模式分析
//!
//! 深信服可以通过以下特征识别 VPN 流量：
//! 1. 持续均匀的流量模式（正常浏览是突发性的）
//! 2. 固定大小的数据包
//! 3. 规律的时间间隔
//! 4. 长时间的持续连接
//!
//! 本模块通过以下技术对抗：
//! 1. 随机延迟注入 - 模拟人类浏览行为
//! 2. 数据包大小随机化 - 打破固定包长特征
//! 3. 虚假流量注入 - 混淆真实流量模式
//! 4. 流量突发模拟 - 模拟网页加载的突发特征

use std::time::Duration;
use rand::Rng;
use tokio::time::{sleep, Instant};

/// 流量整形配置
#[derive(Clone, Debug)]
pub struct TrafficShapingConfig {
    /// 是否启用流量整形
    pub enabled: bool,
    /// 最小延迟 (毫秒)
    pub min_delay_ms: u32,
    /// 最大延迟 (毫秒)
    pub max_delay_ms: u32,
    /// 是否启用数据包填充
    pub enable_padding: bool,
    /// 最小填充大小
    pub min_padding: usize,
    /// 最大填充大小
    pub max_padding: usize,
    /// 是否启用虚假流量
    pub enable_dummy_traffic: bool,
    /// 虚假流量发送间隔 (秒)
    pub dummy_interval_secs: u32,
    /// 流量模式类型
    pub pattern: TrafficPattern,
}

impl Default for TrafficShapingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_delay_ms: 0,
            max_delay_ms: 50,
            enable_padding: true,
            min_padding: 0,
            max_padding: 256,
            enable_dummy_traffic: false,
            dummy_interval_secs: 30,
            pattern: TrafficPattern::WebBrowsing,
        }
    }
}

/// 流量模式类型
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrafficPattern {
    /// 网页浏览模式 - 突发性流量，有明显的请求-响应模式
    WebBrowsing,
    /// 视频流模式 - 持续但有缓冲波动
    VideoStreaming,
    /// 文件下载模式 - 持续高速
    FileDownload,
    /// 随机模式 - 完全随机化
    Random,
    /// 自适应模式 - 根据实际流量自动调整
    Adaptive,
}

/// 流量整形器
pub struct TrafficShaper {
    config: TrafficShapingConfig,
    last_send_time: Instant,
    bytes_sent: u64,
    packets_sent: u64,
    burst_state: BurstState,
}

/// 突发状态
struct BurstState {
    /// 是否在突发期
    in_burst: bool,
    /// 突发开始时间
    burst_start: Instant,
    /// 突发持续时间
    burst_duration: Duration,
    /// 下次突发时间
    next_burst: Instant,
}

impl TrafficShaper {
    pub fn new(config: TrafficShapingConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            last_send_time: now,
            bytes_sent: 0,
            packets_sent: 0,
            burst_state: BurstState {
                in_burst: true,
                burst_start: now,
                burst_duration: Duration::from_millis(500),
                next_burst: now + Duration::from_secs(2),
            },
        }
    }

    /// 在发送数据前调用，返回应该等待的时间
    pub async fn shape_outgoing(&mut self, data_len: usize) -> Duration {
        if !self.config.enabled {
            return Duration::ZERO;
        }

        let delay = match self.config.pattern {
            TrafficPattern::WebBrowsing => self.web_browsing_delay(data_len),
            TrafficPattern::VideoStreaming => self.video_streaming_delay(data_len),
            TrafficPattern::FileDownload => self.file_download_delay(),
            TrafficPattern::Random => self.random_delay(),
            TrafficPattern::Adaptive => self.adaptive_delay(data_len),
        };

        if delay > Duration::ZERO {
            sleep(delay).await;
        }

        self.last_send_time = Instant::now();
        self.bytes_sent += data_len as u64;
        self.packets_sent += 1;

        delay
    }

    /// 网页浏览模式延迟
    /// 特点：突发性请求，然后等待响应，再突发
    fn web_browsing_delay(&mut self, _data_len: usize) -> Duration {
        let mut rng = rand::thread_rng();
        let now = Instant::now();

        // 更新突发状态
        if self.burst_state.in_burst {
            if now > self.burst_state.burst_start + self.burst_state.burst_duration {
                // 突发结束，进入等待期
                self.burst_state.in_burst = false;
                self.burst_state.next_burst = now + Duration::from_millis(
                    rng.gen_range(500..3000)
                );
            }
        } else if now >= self.burst_state.next_burst {
            // 开始新的突发
            self.burst_state.in_burst = true;
            self.burst_state.burst_start = now;
            self.burst_state.burst_duration = Duration::from_millis(
                rng.gen_range(200..1000)
            );
        }

        if self.burst_state.in_burst {
            // 突发期：快速发送，只有很小的随机延迟
            Duration::from_micros(rng.gen_range(0..5000))
        } else {
            // 等待期：较长的延迟
            Duration::from_millis(rng.gen_range(50..200))
        }
    }

    /// 视频流模式延迟
    /// 特点：相对稳定但有周期性波动（模拟缓冲）
    fn video_streaming_delay(&self, _data_len: usize) -> Duration {
        let mut rng = rand::thread_rng();
        
        // 模拟视频缓冲的周期性波动
        let base_delay = 10; // 基础延迟 10ms
        let jitter = rng.gen_range(0..20); // 0-20ms 抖动
        
        // 偶尔模拟缓冲事件（较长延迟）
        if rng.gen_ratio(1, 100) {
            Duration::from_millis(rng.gen_range(100..500))
        } else {
            Duration::from_millis(base_delay + jitter)
        }
    }

    /// 文件下载模式延迟
    /// 特点：持续高速，几乎无延迟
    fn file_download_delay(&self) -> Duration {
        let mut rng = rand::thread_rng();
        // 只有很小的随机延迟
        Duration::from_micros(rng.gen_range(0..1000))
    }

    /// 随机延迟
    fn random_delay(&self) -> Duration {
        let mut rng = rand::thread_rng();
        Duration::from_millis(
            rng.gen_range(self.config.min_delay_ms..=self.config.max_delay_ms) as u64
        )
    }

    /// 自适应延迟
    fn adaptive_delay(&self, data_len: usize) -> Duration {
        let mut rng = rand::thread_rng();
        
        // 根据数据大小调整延迟
        // 小数据包（可能是控制包）：较长延迟
        // 大数据包（可能是数据传输）：较短延迟
        let base_delay = if data_len < 100 {
            rng.gen_range(10..50)
        } else if data_len < 1000 {
            rng.gen_range(5..20)
        } else {
            rng.gen_range(0..10)
        };

        Duration::from_millis(base_delay)
    }

    /// 生成填充数据
    pub fn generate_padding(&self) -> Vec<u8> {
        if !self.config.enable_padding {
            return Vec::new();
        }

        let mut rng = rand::thread_rng();
        let padding_len = rng.gen_range(self.config.min_padding..=self.config.max_padding);
        
        // 生成看起来像随机加密数据的填充
        let mut padding = vec![0u8; padding_len];
        rng.fill(&mut padding[..]);
        
        padding
    }

    /// 将数据包大小随机化到目标范围
    pub fn randomize_packet_size(&self, data: &[u8], target_sizes: &[usize]) -> Vec<u8> {
        if target_sizes.is_empty() || !self.config.enable_padding {
            return data.to_vec();
        }

        let mut rng = rand::thread_rng();
        let target = target_sizes[rng.gen_range(0..target_sizes.len())];
        
        if data.len() >= target {
            return data.to_vec();
        }

        let mut result = data.to_vec();
        let padding_needed = target - data.len();
        let mut padding = vec![0u8; padding_needed];
        rng.fill(&mut padding[..]);
        result.extend(padding);
        
        result
    }
}

/// 虚假流量生成器
pub struct DummyTrafficGenerator {
    config: TrafficShapingConfig,
    last_dummy_time: Instant,
}

impl DummyTrafficGenerator {
    pub fn new(config: TrafficShapingConfig) -> Self {
        Self {
            config,
            last_dummy_time: Instant::now(),
        }
    }

    /// 检查是否应该发送虚假流量
    pub fn should_send_dummy(&mut self) -> bool {
        if !self.config.enable_dummy_traffic {
            return false;
        }

        let now = Instant::now();
        let interval = Duration::from_secs(self.config.dummy_interval_secs as u64);
        
        if now >= self.last_dummy_time + interval {
            self.last_dummy_time = now;
            true
        } else {
            false
        }
    }

    /// 生成虚假流量数据
    pub fn generate_dummy_packet(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        
        // 生成看起来像真实数据的虚假包
        // 大小模拟常见的 HTTP 响应大小
        let sizes = [64, 128, 256, 512, 1024, 1460];
        let size = sizes[rng.gen_range(0..sizes.len())];
        
        let mut data = vec![0u8; size];
        rng.fill(&mut data[..]);
        
        // 添加一个标记表示这是虚假流量（接收端可以识别并丢弃）
        // 使用特殊的魔数作为前缀
        data[0] = 0xDE;
        data[1] = 0xAD;
        data[2] = 0xBE;
        data[3] = 0xEF;
        
        data
    }

    /// 检查是否是虚假流量
    pub fn is_dummy_packet(data: &[u8]) -> bool {
        data.len() >= 4 
            && data[0] == 0xDE 
            && data[1] == 0xAD 
            && data[2] == 0xBE 
            && data[3] == 0xEF
    }
}

/// 常见 HTTP 响应大小（用于数据包大小伪装）
pub const COMMON_HTTP_SIZES: &[usize] = &[
    64,    // 小型 API 响应
    128,   // 小型 JSON
    256,   // 中型 JSON
    512,   // 较大 JSON
    1024,  // 1KB
    1460,  // TCP MSS
    2048,  // 2KB
    4096,  // 4KB
    8192,  // 8KB
    16384, // 16KB
];

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_traffic_shaper() {
        let config = TrafficShapingConfig::default();
        let mut shaper = TrafficShaper::new(config);
        
        // 测试延迟生成
        for _ in 0..10 {
            let delay = shaper.shape_outgoing(1000).await;
            assert!(delay <= Duration::from_millis(100));
        }
    }

    #[test]
    fn test_padding_generation() {
        let config = TrafficShapingConfig {
            enable_padding: true,
            min_padding: 10,
            max_padding: 100,
            ..Default::default()
        };
        let shaper = TrafficShaper::new(config);
        
        let padding = shaper.generate_padding();
        assert!(padding.len() >= 10 && padding.len() <= 100);
    }

    #[test]
    fn test_dummy_traffic() {
        let config = TrafficShapingConfig {
            enable_dummy_traffic: true,
            dummy_interval_secs: 1,
            ..Default::default()
        };
        let generator = DummyTrafficGenerator::new(config);
        
        let dummy = generator.generate_dummy_packet();
        assert!(DummyTrafficGenerator::is_dummy_packet(&dummy));
        
        let real_data = vec![1, 2, 3, 4, 5];
        assert!(!DummyTrafficGenerator::is_dummy_packet(&real_data));
    }
}
