//! 对抗机器学习模块 - 2024-2025 前沿技术
//!
//! 原理：
//! 1. 深信服可能使用 ML 模型识别 VPN 流量
//! 2. 通过注入对抗样本扰动数据包
//! 3. 使 ML 分类器产生错误判断
//! 4. 类似于图像对抗样本攻击
//!
//! 2025 新增：
//! - Markov 链流量生成：基于真实流量统计生成更逼真的伪装流量
//! - 时间侧信道防护：随机延迟注入，打破时间模式
//! - 自适应对抗：根据检测反馈动态调整策略
//!
//! 这是最前沿的反审查技术

use rand::Rng;
use std::collections::{VecDeque, HashMap};
use std::time::{Duration, Instant};

/// 对抗 ML 配置
#[derive(Clone, Debug)]
pub struct AdversarialConfig {
    /// 是否启用
    pub enabled: bool,
    /// 扰动强度 (0.0 - 1.0)
    pub perturbation_strength: f64,
    /// 扰动模式
    pub mode: AdversarialMode,
    /// 是否启用流量模仿
    pub traffic_mimicry: bool,
    /// 模仿目标
    pub mimicry_target: MimicryTarget,
    /// 特征混淆级别
    pub obfuscation_level: u8,
}

impl Default for AdversarialConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            perturbation_strength: 0.1,
            mode: AdversarialMode::Adaptive,
            traffic_mimicry: true,
            mimicry_target: MimicryTarget::WebBrowsing,
            obfuscation_level: 3,
        }
    }
}

/// 对抗模式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AdversarialMode {
    /// 随机扰动
    Random,
    /// 梯度攻击（模拟）
    GradientBased,
    /// 自适应
    Adaptive,
    /// 特征擦除
    FeatureErasure,
    /// 特征注入
    FeatureInjection,
}

/// 模仿目标
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MimicryTarget {
    /// 网页浏览
    WebBrowsing,
    /// 视频流
    VideoStreaming,
    /// 文件下载
    FileDownload,
    /// 在线游戏
    OnlineGaming,
    /// 视频会议
    VideoConference,
}

/// 流量特征
#[derive(Clone, Debug)]
pub struct TrafficFeatures {
    /// 包大小分布
    pub packet_sizes: Vec<usize>,
    /// 包间隔分布 (微秒)
    pub inter_arrival_times: Vec<u64>,
    /// 上行/下行比例
    pub up_down_ratio: f64,
    /// 突发特征
    pub burst_size: usize,
    /// 流持续时间
    pub flow_duration: u64,
}

impl TrafficFeatures {
    /// 网页浏览特征
    pub fn web_browsing() -> Self {
        Self {
            packet_sizes: vec![64, 128, 256, 512, 1024, 1460],
            inter_arrival_times: vec![1000, 5000, 10000, 50000, 100000],
            up_down_ratio: 0.3,
            burst_size: 10,
            flow_duration: 5000000, // 5 秒
        }
    }

    /// 视频流特征
    pub fn video_streaming() -> Self {
        Self {
            packet_sizes: vec![1200, 1300, 1400, 1460],
            inter_arrival_times: vec![10000, 15000, 20000, 33000],
            up_down_ratio: 0.05,
            burst_size: 30,
            flow_duration: 300000000, // 5 分钟
        }
    }

    /// 视频会议特征
    pub fn video_conference() -> Self {
        Self {
            packet_sizes: vec![100, 200, 300, 1200, 1300],
            inter_arrival_times: vec![20000, 33000, 40000],
            up_down_ratio: 0.8,
            burst_size: 5,
            flow_duration: 1800000000, // 30 分钟
        }
    }
}

/// 对抗样本生成器
pub struct AdversarialGenerator {
    config: AdversarialConfig,
    target_features: TrafficFeatures,
    packet_history: VecDeque<PacketInfo>,
    perturbation_cache: Vec<Vec<u8>>,
}

/// 包信息
#[derive(Clone, Debug)]
struct PacketInfo {
    size: usize,
    timestamp: u64,
    direction: bool, // true = 上行
}

impl AdversarialGenerator {
    pub fn new(config: AdversarialConfig) -> Self {
        let target_features = match config.mimicry_target {
            MimicryTarget::WebBrowsing => TrafficFeatures::web_browsing(),
            MimicryTarget::VideoStreaming => TrafficFeatures::video_streaming(),
            MimicryTarget::VideoConference => TrafficFeatures::video_conference(),
            _ => TrafficFeatures::web_browsing(),
        };
        
        Self {
            config,
            target_features,
            packet_history: VecDeque::with_capacity(1000),
            perturbation_cache: Vec::new(),
        }
    }

    /// 处理出站数据包
    pub fn process_outbound(&mut self, data: &[u8]) -> Vec<u8> {
        if !self.config.enabled {
            return data.to_vec();
        }

        let mut result = data.to_vec();

        match self.config.mode {
            AdversarialMode::Random => {
                self.apply_random_perturbation(&mut result);
            }
            AdversarialMode::GradientBased => {
                self.apply_gradient_perturbation(&mut result);
            }
            AdversarialMode::Adaptive => {
                self.apply_adaptive_perturbation(&mut result);
            }
            AdversarialMode::FeatureErasure => {
                self.erase_features(&mut result);
            }
            AdversarialMode::FeatureInjection => {
                self.inject_features(&mut result);
            }
        }

        // 记录包信息
        self.packet_history.push_back(PacketInfo {
            size: result.len(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            direction: true,
        });

        if self.packet_history.len() > 1000 {
            self.packet_history.pop_front();
        }

        result
    }

    /// 随机扰动
    fn apply_random_perturbation(&self, data: &mut Vec<u8>) {
        let mut rng = rand::thread_rng();
        let strength = self.config.perturbation_strength;
        
        // 随机修改一些字节
        let num_changes = (data.len() as f64 * strength * 0.01) as usize;
        for _ in 0..num_changes {
            if !data.is_empty() {
                let idx = rng.gen_range(0..data.len());
                data[idx] ^= rng.gen::<u8>();
            }
        }
        
        // 随机填充
        if rng.gen_bool(strength) {
            let padding_len = rng.gen_range(1..64);
            let mut padding = vec![0u8; padding_len];
            rng.fill(&mut padding[..]);
            data.extend(padding);
        }
    }

    /// 梯度攻击（模拟）
    fn apply_gradient_perturbation(&self, data: &mut Vec<u8>) {
        // 模拟 FGSM (Fast Gradient Sign Method) 攻击
        // 实际实现需要知道目标模型的梯度
        
        let mut rng = rand::thread_rng();
        let epsilon = (self.config.perturbation_strength * 10.0) as u8;
        
        // 对每个字节应用扰动
        for byte in data.iter_mut() {
            let sign: i16 = if rng.gen_bool(0.5) { 1 } else { -1 };
            let new_val = (*byte as i16) + sign * (epsilon as i16);
            *byte = new_val.clamp(0, 255) as u8;
        }
    }

    /// 自适应扰动
    fn apply_adaptive_perturbation(&mut self, data: &mut Vec<u8>) {
        let mut rng = rand::thread_rng();
        
        // 分析当前流量特征
        let current_features = self.analyze_current_features();
        
        // 计算与目标特征的差异
        let size_diff = self.calculate_size_difference(&current_features);
        
        // 调整包大小以匹配目标分布
        if size_diff > 0.1 {
            let target_size = self.target_features.packet_sizes
                [rng.gen_range(0..self.target_features.packet_sizes.len())];
            
            if data.len() < target_size {
                // 填充到目标大小
                let padding = target_size - data.len();
                let mut pad_data = vec![0u8; padding];
                rng.fill(&mut pad_data[..]);
                data.extend(pad_data);
            }
        }
        
        // 添加噪声
        self.apply_random_perturbation(data);
    }

    /// 特征擦除
    fn erase_features(&self, data: &mut Vec<u8>) {
        let mut rng = rand::thread_rng();
        
        // 擦除可能被用于识别的特征
        
        // 1. 随机化包大小
        let target_sizes = &self.target_features.packet_sizes;
        let target = target_sizes[rng.gen_range(0..target_sizes.len())];
        
        if data.len() < target {
            let padding = target - data.len();
            data.extend(vec![rng.gen::<u8>(); padding]);
        } else if data.len() > target && data.len() > 100 {
            // 不能截断太多，保留至少 100 字节
            data.truncate(target.max(100));
        }
        
        // 2. 添加随机头部
        let header_len = rng.gen_range(4..16);
        let mut header = vec![0u8; header_len];
        rng.fill(&mut header[..]);
        
        let mut new_data = header;
        new_data.extend_from_slice(data);
        *data = new_data;
    }

    /// 特征注入
    fn inject_features(&self, data: &mut Vec<u8>) {
        let mut rng = rand::thread_rng();
        
        // 注入看起来像正常流量的特征
        
        // 1. 添加 HTTP 风格的头部
        if rng.gen_bool(0.3) {
            let http_headers = [
                b"GET / HTTP/1.1\r\n".as_slice(),
                b"POST /api HTTP/1.1\r\n".as_slice(),
                b"HTTP/1.1 200 OK\r\n".as_slice(),
            ];
            let header = http_headers[rng.gen_range(0..http_headers.len())];
            
            let mut new_data = header.to_vec();
            new_data.extend_from_slice(data);
            *data = new_data;
        }
        
        // 2. 添加 TLS 记录头
        if rng.gen_bool(0.3) {
            let tls_header = [
                0x17, // Application Data
                0x03, 0x03, // TLS 1.2
                ((data.len() >> 8) & 0xff) as u8,
                (data.len() & 0xff) as u8,
            ];
            
            let mut new_data = tls_header.to_vec();
            new_data.extend_from_slice(data);
            *data = new_data;
        }
    }

    /// 分析当前流量特征
    fn analyze_current_features(&self) -> TrafficFeatures {
        let sizes: Vec<usize> = self.packet_history.iter()
            .map(|p| p.size)
            .collect();
        
        let times: Vec<u64> = self.packet_history.iter()
            .map(|p| p.timestamp)
            .collect();
        
        let inter_arrival: Vec<u64> = times.windows(2)
            .map(|w| w[1].saturating_sub(w[0]))
            .collect();
        
        let up_count = self.packet_history.iter()
            .filter(|p| p.direction)
            .count();
        
        TrafficFeatures {
            packet_sizes: sizes,
            inter_arrival_times: inter_arrival,
            up_down_ratio: up_count as f64 / self.packet_history.len().max(1) as f64,
            burst_size: 0,
            flow_duration: 0,
        }
    }

    /// 计算大小分布差异
    fn calculate_size_difference(&self, current: &TrafficFeatures) -> f64 {
        if current.packet_sizes.is_empty() {
            return 1.0;
        }
        
        let current_avg: f64 = current.packet_sizes.iter()
            .map(|&s| s as f64)
            .sum::<f64>() / current.packet_sizes.len() as f64;
        
        let target_avg: f64 = self.target_features.packet_sizes.iter()
            .map(|&s| s as f64)
            .sum::<f64>() / self.target_features.packet_sizes.len() as f64;
        
        (current_avg - target_avg).abs() / target_avg
    }

    /// 生成诱饵流量
    pub fn generate_decoy_traffic(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        
        // 选择目标大小
        let size = self.target_features.packet_sizes
            [rng.gen_range(0..self.target_features.packet_sizes.len())];
        
        let mut data = vec![0u8; size];
        rng.fill(&mut data[..]);
        
        // 添加看起来像真实流量的特征
        match self.config.mimicry_target {
            MimicryTarget::WebBrowsing => {
                // HTTP 响应风格
                data[0..4].copy_from_slice(b"HTTP");
            }
            MimicryTarget::VideoStreaming => {
                // RTP 风格
                data[0] = 0x80; // RTP version 2
                data[1] = 96;   // Payload type
            }
            MimicryTarget::VideoConference => {
                // WebRTC 风格
                data[0] = 0x80;
                data[1] = 111; // Opus
            }
            _ => {}
        }
        
        data
    }
}

/// 流量分类器对抗
pub struct ClassifierAdversary {
    /// 已知的分类器特征
    known_features: Vec<String>,
    /// 对抗策略
    strategies: Vec<AdversarialStrategy>,
}

/// 对抗策略
#[derive(Clone, Debug)]
pub struct AdversarialStrategy {
    pub name: String,
    pub target_feature: String,
    pub action: StrategyAction,
}

#[derive(Clone, Debug)]
pub enum StrategyAction {
    /// 随机化
    Randomize,
    /// 模仿
    Mimic(String),
    /// 擦除
    Erase,
    /// 注入噪声
    InjectNoise(f64),
}

impl ClassifierAdversary {
    pub fn new() -> Self {
        Self {
            known_features: vec![
                "packet_size_distribution".to_string(),
                "inter_arrival_time".to_string(),
                "flow_duration".to_string(),
                "byte_distribution".to_string(),
                "protocol_fingerprint".to_string(),
            ],
            strategies: Vec::new(),
        }
    }

    /// 添加对抗策略
    pub fn add_strategy(&mut self, strategy: AdversarialStrategy) {
        self.strategies.push(strategy);
    }

    /// 应用所有策略
    pub fn apply_strategies(&self, data: &mut Vec<u8>) {
        for strategy in &self.strategies {
            match &strategy.action {
                StrategyAction::Randomize => {
                    let mut rng = rand::thread_rng();
                    for byte in data.iter_mut() {
                        if rng.gen_bool(0.01) {
                            *byte = rng.gen();
                        }
                    }
                }
                StrategyAction::InjectNoise(strength) => {
                    let mut rng = rand::thread_rng();
                    for byte in data.iter_mut() {
                        if rng.gen_bool(*strength) {
                            *byte ^= rng.gen::<u8>() & 0x0f;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

impl Default for ClassifierAdversary {
    fn default() -> Self {
        Self::new()
    }
}

/// Markov 链流量生成器 - 2025 新增
/// 基于真实流量统计生成更逼真的伪装流量
pub struct MarkovTrafficGenerator {
    /// 包大小转移矩阵
    size_transitions: HashMap<usize, Vec<(usize, f64)>>,
    /// 时间间隔转移矩阵
    time_transitions: HashMap<u64, Vec<(u64, f64)>>,
    /// 当前状态
    current_size: usize,
    current_time: u64,
    /// 是否已训练
    trained: bool,
    /// 历史数据
    size_history: Vec<usize>,
    time_history: Vec<u64>,
}

impl MarkovTrafficGenerator {
    pub fn new() -> Self {
        Self {
            size_transitions: HashMap::new(),
            time_transitions: HashMap::new(),
            current_size: 100,
            current_time: 10000,
            trained: false,
            size_history: Vec::new(),
            time_history: Vec::new(),
        }
    }

    /// 从预设的流量模式初始化
    pub fn from_preset(target: MimicryTarget) -> Self {
        let mut gen = Self::new();
        
        match target {
            MimicryTarget::WebBrowsing => {
                gen.init_web_browsing_model();
            }
            MimicryTarget::VideoStreaming => {
                gen.init_video_streaming_model();
            }
            MimicryTarget::VideoConference => {
                gen.init_video_conference_model();
            }
            _ => {
                gen.init_web_browsing_model();
            }
        }
        
        gen.trained = true;
        gen
    }

    /// 初始化网页浏览模型
    fn init_web_browsing_model(&mut self) {
        // 典型的网页浏览包大小转移
        // 小包 -> 大包（请求后响应）
        // 大包 -> 小包（响应后新请求）
        self.size_transitions.insert(64, vec![
            (64, 0.2), (128, 0.3), (256, 0.2), (512, 0.15), (1460, 0.15)
        ]);
        self.size_transitions.insert(128, vec![
            (64, 0.1), (128, 0.2), (256, 0.3), (512, 0.2), (1460, 0.2)
        ]);
        self.size_transitions.insert(256, vec![
            (64, 0.15), (128, 0.15), (256, 0.2), (512, 0.25), (1460, 0.25)
        ]);
        self.size_transitions.insert(512, vec![
            (64, 0.2), (128, 0.2), (256, 0.2), (512, 0.2), (1460, 0.2)
        ]);
        self.size_transitions.insert(1460, vec![
            (64, 0.3), (128, 0.2), (256, 0.2), (512, 0.15), (1460, 0.15)
        ]);

        // 时间间隔转移（微秒）
        self.time_transitions.insert(1000, vec![
            (1000, 0.3), (5000, 0.3), (10000, 0.2), (50000, 0.1), (100000, 0.1)
        ]);
        self.time_transitions.insert(5000, vec![
            (1000, 0.2), (5000, 0.3), (10000, 0.25), (50000, 0.15), (100000, 0.1)
        ]);
        self.time_transitions.insert(10000, vec![
            (1000, 0.15), (5000, 0.25), (10000, 0.3), (50000, 0.2), (100000, 0.1)
        ]);
        self.time_transitions.insert(50000, vec![
            (1000, 0.1), (5000, 0.2), (10000, 0.3), (50000, 0.25), (100000, 0.15)
        ]);
        self.time_transitions.insert(100000, vec![
            (1000, 0.2), (5000, 0.2), (10000, 0.2), (50000, 0.2), (100000, 0.2)
        ]);
    }

    /// 初始化视频流模型
    fn init_video_streaming_model(&mut self) {
        // 视频流特点：大包为主，间隔稳定
        self.size_transitions.insert(1200, vec![
            (1200, 0.4), (1300, 0.3), (1400, 0.2), (1460, 0.1)
        ]);
        self.size_transitions.insert(1300, vec![
            (1200, 0.3), (1300, 0.4), (1400, 0.2), (1460, 0.1)
        ]);
        self.size_transitions.insert(1400, vec![
            (1200, 0.2), (1300, 0.3), (1400, 0.35), (1460, 0.15)
        ]);
        self.size_transitions.insert(1460, vec![
            (1200, 0.25), (1300, 0.25), (1400, 0.25), (1460, 0.25)
        ]);

        // 视频流时间间隔更稳定
        self.time_transitions.insert(10000, vec![
            (10000, 0.4), (15000, 0.3), (20000, 0.2), (33000, 0.1)
        ]);
        self.time_transitions.insert(15000, vec![
            (10000, 0.3), (15000, 0.4), (20000, 0.2), (33000, 0.1)
        ]);
        self.time_transitions.insert(20000, vec![
            (10000, 0.2), (15000, 0.3), (20000, 0.35), (33000, 0.15)
        ]);
        self.time_transitions.insert(33000, vec![
            (10000, 0.25), (15000, 0.25), (20000, 0.25), (33000, 0.25)
        ]);
    }

    /// 初始化视频会议模型
    fn init_video_conference_model(&mut self) {
        // 视频会议：双向流量，包大小变化大
        self.size_transitions.insert(100, vec![
            (100, 0.3), (200, 0.25), (300, 0.2), (1200, 0.15), (1300, 0.1)
        ]);
        self.size_transitions.insert(200, vec![
            (100, 0.25), (200, 0.3), (300, 0.2), (1200, 0.15), (1300, 0.1)
        ]);
        self.size_transitions.insert(300, vec![
            (100, 0.2), (200, 0.25), (300, 0.25), (1200, 0.2), (1300, 0.1)
        ]);
        self.size_transitions.insert(1200, vec![
            (100, 0.15), (200, 0.15), (300, 0.2), (1200, 0.3), (1300, 0.2)
        ]);
        self.size_transitions.insert(1300, vec![
            (100, 0.2), (200, 0.2), (300, 0.2), (1200, 0.2), (1300, 0.2)
        ]);

        // 视频会议时间间隔
        self.time_transitions.insert(20000, vec![
            (20000, 0.4), (33000, 0.35), (40000, 0.25)
        ]);
        self.time_transitions.insert(33000, vec![
            (20000, 0.35), (33000, 0.4), (40000, 0.25)
        ]);
        self.time_transitions.insert(40000, vec![
            (20000, 0.3), (33000, 0.35), (40000, 0.35)
        ]);
    }

    /// 从观察到的流量学习
    pub fn learn(&mut self, packet_size: usize, inter_arrival_us: u64) {
        self.size_history.push(packet_size);
        self.time_history.push(inter_arrival_us);
        
        // 当收集足够数据时，更新转移矩阵
        if self.size_history.len() >= 100 {
            self.update_transitions();
            self.trained = true;
        }
    }

    /// 更新转移矩阵
    fn update_transitions(&mut self) {
        // 量化包大小到离散状态
        let quantized_sizes: Vec<usize> = self.size_history.iter()
            .map(|&s| self.quantize_size(s))
            .collect();
        
        // 计算转移概率
        let mut size_counts: HashMap<usize, HashMap<usize, usize>> = HashMap::new();
        for window in quantized_sizes.windows(2) {
            let from = window[0];
            let to = window[1];
            *size_counts.entry(from).or_default().entry(to).or_insert(0) += 1;
        }
        
        // 转换为概率
        for (from, to_counts) in size_counts {
            let total: usize = to_counts.values().sum();
            let probs: Vec<(usize, f64)> = to_counts.into_iter()
                .map(|(to, count)| (to, count as f64 / total as f64))
                .collect();
            self.size_transitions.insert(from, probs);
        }
        
        // 类似处理时间间隔
        let quantized_times: Vec<u64> = self.time_history.iter()
            .map(|&t| self.quantize_time(t))
            .collect();
        
        let mut time_counts: HashMap<u64, HashMap<u64, usize>> = HashMap::new();
        for window in quantized_times.windows(2) {
            let from = window[0];
            let to = window[1];
            *time_counts.entry(from).or_default().entry(to).or_insert(0) += 1;
        }
        
        for (from, to_counts) in time_counts {
            let total: usize = to_counts.values().sum();
            let probs: Vec<(u64, f64)> = to_counts.into_iter()
                .map(|(to, count)| (to, count as f64 / total as f64))
                .collect();
            self.time_transitions.insert(from, probs);
        }
    }

    /// 量化包大小
    fn quantize_size(&self, size: usize) -> usize {
        match size {
            0..=96 => 64,
            97..=192 => 128,
            193..=384 => 256,
            385..=768 => 512,
            769..=1200 => 1024,
            _ => 1460,
        }
    }

    /// 量化时间间隔
    fn quantize_time(&self, time_us: u64) -> u64 {
        match time_us {
            0..=2500 => 1000,
            2501..=7500 => 5000,
            7501..=25000 => 10000,
            25001..=75000 => 50000,
            _ => 100000,
        }
    }

    /// 生成下一个包大小
    pub fn next_size(&mut self) -> usize {
        let transitions = self.size_transitions.get(&self.current_size)
            .cloned()
            .unwrap_or_else(|| vec![(self.current_size, 1.0)]);
        
        self.current_size = self.sample_transition(&transitions);
        self.current_size
    }

    /// 生成下一个时间间隔（微秒）
    pub fn next_interval(&mut self) -> u64 {
        let transitions = self.time_transitions.get(&self.current_time)
            .cloned()
            .unwrap_or_else(|| vec![(self.current_time, 1.0)]);
        
        self.current_time = self.sample_transition(&transitions);
        self.current_time
    }

    /// 根据概率采样
    fn sample_transition<T: Copy>(&self, transitions: &[(T, f64)]) -> T {
        let mut rng = rand::thread_rng();
        let r: f64 = rng.gen();
        let mut cumulative = 0.0;
        
        for &(state, prob) in transitions {
            cumulative += prob;
            if r < cumulative {
                return state;
            }
        }
        
        transitions.last().map(|&(s, _)| s).unwrap_or_else(|| transitions[0].0)
    }

    /// 生成一个完整的流量序列
    pub fn generate_sequence(&mut self, count: usize) -> Vec<(usize, u64)> {
        (0..count).map(|_| (self.next_size(), self.next_interval())).collect()
    }
}

impl Default for MarkovTrafficGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// 时间侧信道防护 - 2025 新增
pub struct TimingDefense {
    /// 最小延迟（微秒）
    min_delay_us: u64,
    /// 最大延迟（微秒）
    max_delay_us: u64,
    /// 是否启用恒定速率模式
    constant_rate: bool,
    /// 恒定速率间隔（微秒）
    rate_interval_us: u64,
    /// 上次发送时间
    last_send: Option<Instant>,
    /// 延迟抖动
    jitter_factor: f64,
}

impl TimingDefense {
    pub fn new() -> Self {
        Self {
            min_delay_us: 1000,
            max_delay_us: 10000,
            constant_rate: false,
            rate_interval_us: 5000,
            last_send: None,
            jitter_factor: 0.2,
        }
    }

    /// 创建恒定速率模式
    pub fn constant_rate(interval_us: u64) -> Self {
        Self {
            min_delay_us: interval_us,
            max_delay_us: interval_us,
            constant_rate: true,
            rate_interval_us: interval_us,
            last_send: None,
            jitter_factor: 0.1,
        }
    }

    /// 计算下一次发送应该等待的时间
    pub fn get_delay(&mut self) -> Duration {
        let mut rng = rand::thread_rng();
        
        let base_delay = if self.constant_rate {
            self.rate_interval_us
        } else {
            rng.gen_range(self.min_delay_us..=self.max_delay_us)
        };
        
        // 添加抖动
        let jitter = (base_delay as f64 * self.jitter_factor * (rng.gen::<f64>() - 0.5) * 2.0) as i64;
        let final_delay = (base_delay as i64 + jitter).max(0) as u64;
        
        // 考虑上次发送时间
        if let Some(last) = self.last_send {
            let elapsed = last.elapsed().as_micros() as u64;
            if elapsed < final_delay {
                return Duration::from_micros(final_delay - elapsed);
            }
        }
        
        self.last_send = Some(Instant::now());
        Duration::from_micros(final_delay)
    }

    /// 记录发送
    pub fn record_send(&mut self) {
        self.last_send = Some(Instant::now());
    }
}

impl Default for TimingDefense {
    fn default() -> Self {
        Self::new()
    }
}

/// 自适应对抗策略 - 2025 新增
pub struct AdaptiveAdversary {
    /// 当前策略
    current_strategy: AdversarialMode,
    /// 策略效果评分
    strategy_scores: HashMap<String, f64>,
    /// 检测计数
    detection_count: u32,
    /// 成功计数
    success_count: u32,
    /// 策略切换阈值
    switch_threshold: f64,
}

impl AdaptiveAdversary {
    pub fn new() -> Self {
        let mut scores = HashMap::new();
        scores.insert("random".to_string(), 0.5);
        scores.insert("gradient".to_string(), 0.5);
        scores.insert("adaptive".to_string(), 0.5);
        scores.insert("feature_erasure".to_string(), 0.5);
        scores.insert("feature_injection".to_string(), 0.5);
        
        Self {
            current_strategy: AdversarialMode::Adaptive,
            strategy_scores: scores,
            detection_count: 0,
            success_count: 0,
            switch_threshold: 0.3,
        }
    }

    /// 报告检测事件
    pub fn report_detection(&mut self) {
        self.detection_count += 1;
        
        // 降低当前策略评分
        let key = self.strategy_key();
        if let Some(score) = self.strategy_scores.get_mut(&key) {
            *score = (*score * 0.9).max(0.1);
        }
        
        // 检查是否需要切换策略
        if self.should_switch() {
            self.switch_strategy();
        }
    }

    /// 报告成功事件
    pub fn report_success(&mut self) {
        self.success_count += 1;
        
        // 提高当前策略评分
        let key = self.strategy_key();
        if let Some(score) = self.strategy_scores.get_mut(&key) {
            *score = (*score * 1.1).min(1.0);
        }
    }

    /// 获取当前策略键
    fn strategy_key(&self) -> String {
        match self.current_strategy {
            AdversarialMode::Random => "random".to_string(),
            AdversarialMode::GradientBased => "gradient".to_string(),
            AdversarialMode::Adaptive => "adaptive".to_string(),
            AdversarialMode::FeatureErasure => "feature_erasure".to_string(),
            AdversarialMode::FeatureInjection => "feature_injection".to_string(),
        }
    }

    /// 是否应该切换策略
    fn should_switch(&self) -> bool {
        let key = self.strategy_key();
        self.strategy_scores.get(&key).copied().unwrap_or(0.5) < self.switch_threshold
    }

    /// 切换到最佳策略
    fn switch_strategy(&mut self) {
        let best = self.strategy_scores.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, _)| k.clone());
        
        if let Some(key) = best {
            self.current_strategy = match key.as_str() {
                "random" => AdversarialMode::Random,
                "gradient" => AdversarialMode::GradientBased,
                "feature_erasure" => AdversarialMode::FeatureErasure,
                "feature_injection" => AdversarialMode::FeatureInjection,
                _ => AdversarialMode::Adaptive,
            };
        }
    }

    /// 获取当前策略
    pub fn get_strategy(&self) -> AdversarialMode {
        self.current_strategy
    }

    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        let total = self.detection_count + self.success_count;
        if total == 0 {
            0.5
        } else {
            self.success_count as f64 / total as f64
        }
    }
}

impl Default for AdaptiveAdversary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adversarial_generator() {
        let config = AdversarialConfig::default();
        let mut generator = AdversarialGenerator::new(config);
        
        let data = vec![1, 2, 3, 4, 5];
        let processed = generator.process_outbound(&data);
        
        // 处理后的数据应该不同
        assert_ne!(data, processed);
    }

    #[test]
    fn test_decoy_traffic() {
        let config = AdversarialConfig::default();
        let generator = AdversarialGenerator::new(config);
        
        let decoy = generator.generate_decoy_traffic();
        assert!(!decoy.is_empty());
    }

    #[test]
    fn test_classifier_adversary() {
        let mut adversary = ClassifierAdversary::new();
        
        adversary.add_strategy(AdversarialStrategy {
            name: "randomize_bytes".to_string(),
            target_feature: "byte_distribution".to_string(),
            action: StrategyAction::Randomize,
        });
        
        let mut data = vec![0u8; 100];
        adversary.apply_strategies(&mut data);
        
        // 数据应该被修改
        assert!(data.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_markov_generator_preset() {
        let mut gen = MarkovTrafficGenerator::from_preset(MimicryTarget::WebBrowsing);
        
        // 生成序列
        let sequence = gen.generate_sequence(10);
        assert_eq!(sequence.len(), 10);
        
        // 验证生成的值在合理范围内
        for (size, interval) in &sequence {
            assert!(*size > 0 && *size <= 1500);
            assert!(*interval > 0);
        }
    }

    #[test]
    fn test_markov_generator_learning() {
        let mut gen = MarkovTrafficGenerator::new();
        
        // 模拟学习过程
        for i in 0..150 {
            let size = 100 + (i % 5) * 100;
            let interval = 1000 + (i % 3) as u64 * 5000;
            gen.learn(size, interval);
        }
        
        assert!(gen.trained);
        
        // 生成应该能工作
        let size = gen.next_size();
        let interval = gen.next_interval();
        assert!(size > 0);
        assert!(interval > 0);
    }

    #[test]
    fn test_timing_defense() {
        let mut defense = TimingDefense::new();
        
        let delay1 = defense.get_delay();
        defense.record_send();
        
        // 延迟应该在合理范围内
        assert!(delay1.as_micros() >= 1000);
        assert!(delay1.as_micros() <= 15000); // 考虑抖动
    }

    #[test]
    fn test_timing_defense_constant_rate() {
        let mut defense = TimingDefense::constant_rate(5000);
        
        let delay = defense.get_delay();
        
        // 恒定速率模式，延迟应该接近设定值
        assert!(delay.as_micros() >= 4000);
        assert!(delay.as_micros() <= 6000);
    }

    #[test]
    fn test_adaptive_adversary() {
        let mut adversary = AdaptiveAdversary::new();
        
        // 初始成功率应该是 0.5
        assert!((adversary.success_rate() - 0.5).abs() < 0.01);
        
        // 报告一些成功
        for _ in 0..10 {
            adversary.report_success();
        }
        
        assert!(adversary.success_rate() > 0.5);
        
        // 报告一些检测
        for _ in 0..20 {
            adversary.report_detection();
        }
        
        // 策略应该已经切换
        assert!(adversary.success_rate() < 0.5);
    }

    #[test]
    fn test_video_streaming_model() {
        let mut gen = MarkovTrafficGenerator::from_preset(MimicryTarget::VideoStreaming);
        
        let sequence = gen.generate_sequence(20);
        
        // 视频流应该主要是大包
        let large_packets = sequence.iter().filter(|(s, _)| *s >= 1000).count();
        assert!(large_packets > sequence.len() / 2);
    }

    #[test]
    fn test_video_conference_model() {
        let mut gen = MarkovTrafficGenerator::from_preset(MimicryTarget::VideoConference);
        
        let sequence = gen.generate_sequence(20);
        
        // 视频会议应该有混合大小的包
        let small_packets = sequence.iter().filter(|(s, _)| *s < 500).count();
        let large_packets = sequence.iter().filter(|(s, _)| *s >= 1000).count();
        
        // 两种大小都应该存在
        assert!(small_packets > 0);
        assert!(large_packets > 0);
    }
}
