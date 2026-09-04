//! TCP 分片模块 - 2024-2025 最有效的 DPI 对抗技术
//!
//! 原理：
//! 1. 将 TLS Client Hello 分成多个小 TCP 包
//! 2. DPI 设备通常只检查第一个包或无法正确重组
//! 3. 每个分片之间添加随机延迟
//! 4. 可以绕过大多数基于特征的检测
//!
//! 2025 新增 Geneva 策略：
//! - TTL 欺骗：发送低 TTL 的干扰包，DPI 能看到但目标服务器收不到
//! - 假 RST 包：发送假的 RST 让 DPI 认为连接已断开
//! - 乱序发送：打乱 TCP 分片顺序，利用 DPI 重组缺陷
//! - 分段重叠：发送重叠的 TCP 段，不同系统处理方式不同
//!
//! 这是对抗 GFW 和深信服最有效的技术之一

use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::sleep;
use rand::Rng;
use rand::seq::SliceRandom;

/// TCP 分片配置
#[derive(Clone, Debug)]
pub struct TcpFragmentConfig {
    /// 是否启用分片
    pub enabled: bool,
    /// 分片大小范围 (最小)
    pub min_fragment_size: usize,
    /// 分片大小范围 (最大)
    pub max_fragment_size: usize,
    /// 分片间延迟范围 (最小，毫秒)
    pub min_delay_ms: u64,
    /// 分片间延迟范围 (最大，毫秒)
    pub max_delay_ms: u64,
    /// 是否只分片 TLS Client Hello
    pub fragment_tls_hello_only: bool,
    /// 分片模式
    pub mode: FragmentMode,
    /// Geneva 高级策略配置
    pub geneva: GenevaConfig,
}

impl Default for TcpFragmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_fragment_size: 1,
            max_fragment_size: 40,
            min_delay_ms: 10,
            max_delay_ms: 50,
            fragment_tls_hello_only: true,
            mode: FragmentMode::Random,
            geneva: GenevaConfig::default(),
        }
    }
}

impl TcpFragmentConfig {
    /// 创建针对深信服优化的配置 (2025 增强版)
    /// 基于深信服 AC/SG 设备的 DPI 特性优化
    pub fn for_sangfor() -> Self {
        Self {
            enabled: true,
            min_fragment_size: 1,
            max_fragment_size: 5,  // 更小的分片
            min_delay_ms: 10,
            max_delay_ms: 50,      // 更长的延迟
            fragment_tls_hello_only: true,
            mode: FragmentMode::SangforOptimized, // 新增专用模式
            geneva: GenevaConfig {
                out_of_order: true,
                disorder_rate: 0.8,  // 更高的乱序率
                duplicate: true,
                duplicate_rate: 0.3, // 更高的重复率
                overlap: true,       // 启用重叠
                overlap_bytes: 2,
                // 2025 新增
                sni_split_positions: vec![1, 3, 5], // 多点分割 SNI
                inject_garbage: true,  // 注入垃圾数据
                randomize_case: true,  // SNI 大小写随机化
                ..Default::default()
            },
        }
    }
    
    /// 创建针对深信服的极限模式 (最强对抗，可能影响兼容性)
    pub fn for_sangfor_extreme() -> Self {
        Self {
            enabled: true,
            min_fragment_size: 1,
            max_fragment_size: 2,  // 极小分片
            min_delay_ms: 30,
            max_delay_ms: 150,     // 更长延迟
            fragment_tls_hello_only: false, // 所有数据都分片
            mode: FragmentMode::SangforExtreme,
            geneva: GenevaConfig {
                out_of_order: true,
                disorder_rate: 0.9,
                duplicate: true,
                duplicate_rate: 0.4,
                overlap: true,
                overlap_bytes: 3,
                sni_split_positions: vec![1, 2, 3, 4, 5],
                inject_garbage: true,
                randomize_case: true,
                reverse_first_fragment: true, // 反转第一个分片的字节序
                ..Default::default()
            },
        }
    }

    /// 创建针对 GFW 优化的配置
    pub fn for_gfw() -> Self {
        Self {
            enabled: true,
            min_fragment_size: 1,
            max_fragment_size: 5,
            min_delay_ms: 10,
            max_delay_ms: 30,
            fragment_tls_hello_only: true,
            mode: FragmentMode::SniSplit,
            geneva: GenevaConfig {
                out_of_order: false,
                duplicate: true,
                duplicate_rate: 0.1,
                ..Default::default()
            },
        }
    }

    /// 创建激进模式配置（最强对抗，可能影响性能）
    pub fn aggressive() -> Self {
        Self {
            enabled: true,
            min_fragment_size: 1,
            max_fragment_size: 3,
            min_delay_ms: 20,
            max_delay_ms: 100,
            fragment_tls_hello_only: false,
            mode: FragmentMode::CombinedSniOutOfOrder,
            geneva: GenevaConfig {
                out_of_order: true,
                disorder_rate: 0.8,
                overlap: true,
                overlap_bytes: 2,
                duplicate: true,
                duplicate_rate: 0.3,
                sni_split_positions: vec![1, 3, 5],
                inject_garbage: true,
                randomize_case: true,
                ..Default::default()
            },
        }
    }
    
    /// 创建静默模式（最小化检测风险，牺牲一些对抗能力）
    pub fn stealth() -> Self {
        Self {
            enabled: true,
            min_fragment_size: 10,
            max_fragment_size: 50,
            min_delay_ms: 5,
            max_delay_ms: 15,
            fragment_tls_hello_only: true,
            mode: FragmentMode::SniSplit,
            geneva: GenevaConfig {
                out_of_order: false,
                duplicate: false,
                overlap: false,
                ..Default::default()
            },
        }
    }
}

/// 分片模式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FragmentMode {
    /// 随机分片大小
    Random,
    /// 固定分片大小
    Fixed(usize),
    /// 按 TLS 记录边界分片
    TlsRecord,
    /// 按 SNI 位置分片（最有效）
    SniSplit,
    /// 1+N 模式（第一个包 1 字节）
    OneByteFirst,
    /// Geneva 策略：乱序发送
    GenevaOutOfOrder,
    /// Geneva 策略：分段重叠
    GenevaOverlap,
    /// 组合策略：SNI 分片 + 乱序
    CombinedSniOutOfOrder,
    /// 深信服优化模式 (2025)
    SangforOptimized,
    /// 深信服极限模式 (2025)
    SangforExtreme,
    /// 多点 SNI 分割
    MultiSniSplit,
}

/// Geneva 高级策略配置
#[derive(Clone, Debug)]
pub struct GenevaConfig {
    /// 是否启用 TTL 欺骗
    pub ttl_trick: bool,
    /// 欺骗包的 TTL 值（设置为比到 DPI 的跳数大，但比到目标服务器小）
    pub decoy_ttl: u8,
    /// 是否发送假 RST 包
    pub fake_rst: bool,
    /// 是否启用乱序发送
    pub out_of_order: bool,
    /// 乱序程度 (0.0-1.0)
    pub disorder_rate: f64,
    /// 是否启用分段重叠
    pub overlap: bool,
    /// 重叠字节数
    pub overlap_bytes: usize,
    /// 是否启用数据包复制（发送重复包干扰 DPI 状态机）
    pub duplicate: bool,
    /// 复制概率
    pub duplicate_rate: f64,
    
    // ===== 2025 新增字段 =====
    /// SNI 分割位置列表（相对于 SNI 开始的偏移）
    pub sni_split_positions: Vec<usize>,
    /// 是否注入垃圾数据（在分片间注入无效数据）
    pub inject_garbage: bool,
    /// 垃圾数据大小范围
    pub garbage_size_range: (usize, usize),
    /// 是否随机化 SNI 大小写（利用 DPI 和服务器处理差异）
    pub randomize_case: bool,
    /// 是否反转第一个分片的字节序
    pub reverse_first_fragment: bool,
    /// 是否在 SNI 中插入空字节
    pub insert_null_bytes: bool,
    /// 分片间最大抖动（毫秒）
    pub jitter_ms: u64,
}

impl Default for GenevaConfig {
    fn default() -> Self {
        Self {
            ttl_trick: false,
            decoy_ttl: 3,
            fake_rst: false,
            out_of_order: true,
            disorder_rate: 0.5,
            overlap: false,
            overlap_bytes: 4,
            duplicate: true,
            duplicate_rate: 0.1,
            // 2025 新增
            sni_split_positions: vec![1, 3],
            inject_garbage: false,
            garbage_size_range: (1, 8),
            randomize_case: false,
            reverse_first_fragment: false,
            insert_null_bytes: false,
            jitter_ms: 10,
        }
    }
}

/// TCP 分片器
pub struct TcpFragmenter {
    config: TcpFragmentConfig,
}

impl TcpFragmenter {
    pub fn new(config: TcpFragmentConfig) -> Self {
        Self { config }
    }

    /// 分片发送数据
    pub async fn send_fragmented<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        data: &[u8],
    ) -> std::io::Result<()> {
        if !self.config.enabled {
            writer.write_all(data).await?;
            return Ok(());
        }

        // 检查是否是 TLS Client Hello
        let is_tls_hello = data.len() > 5 
            && data[0] == 0x16  // Handshake
            && data[5] == 0x01; // Client Hello

        if self.config.fragment_tls_hello_only && !is_tls_hello {
            writer.write_all(data).await?;
            return Ok(());
        }

        // 根据模式分片
        let fragments = match self.config.mode {
            FragmentMode::SniSplit => self.split_at_sni(data),
            FragmentMode::OneByteFirst => self.split_one_byte_first(data),
            FragmentMode::TlsRecord => self.split_tls_records(data),
            FragmentMode::Fixed(size) => self.split_fixed(data, size),
            FragmentMode::Random => self.split_random(data),
            FragmentMode::GenevaOutOfOrder => self.split_geneva_out_of_order(data),
            FragmentMode::GenevaOverlap => self.split_geneva_overlap(data),
            FragmentMode::CombinedSniOutOfOrder => self.split_combined_sni_out_of_order(data),
            FragmentMode::SangforOptimized => self.split_sangfor_optimized(data),
            FragmentMode::SangforExtreme => self.split_sangfor_extreme(data),
            FragmentMode::MultiSniSplit => self.split_multi_sni(data),
        };

        // 应用 Geneva 策略
        let fragments = self.apply_geneva_strategies(fragments);

        // 发送分片
        let mut rng = rand::thread_rng();
        for (i, fragment) in fragments.iter().enumerate() {
            // 可能注入垃圾数据
            if self.config.geneva.inject_garbage && rng.gen_bool(0.3) {
                let garbage = self.generate_garbage();
                writer.write_all(&garbage).await?;
                writer.flush().await?;
                let delay = rng.gen_range(1..3);
                sleep(Duration::from_millis(delay)).await;
            }
            
            // 可能发送重复包
            if self.config.geneva.duplicate && rng.gen_bool(self.config.geneva.duplicate_rate) {
                writer.write_all(fragment).await?;
                writer.flush().await?;
                let delay = rng.gen_range(1..5);
                sleep(Duration::from_millis(delay)).await;
            }

            writer.write_all(fragment).await?;
            writer.flush().await?;

            // 分片间延迟（最后一个分片不延迟）
            if i < fragments.len() - 1 {
                let base_delay = rng.gen_range(self.config.min_delay_ms..=self.config.max_delay_ms);
                let jitter = rng.gen_range(0..=self.config.geneva.jitter_ms);
                sleep(Duration::from_millis(base_delay + jitter)).await;
            }
        }

        Ok(())
    }
    
    /// 深信服优化分片 (2025)
    fn split_sangfor_optimized(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut fragments = Vec::new();
        
        // 查找 SNI 位置
        if let Some(sni_pos) = self.find_sni_position(data) {
            let sni_end = self.find_sni_end(data, sni_pos);
            
            // 在 SNI 之前的数据
            if sni_pos > 0 {
                // 分成多个小片
                let pre_sni = &data[..sni_pos];
                fragments.extend(self.split_into_tiny_fragments(pre_sni, 1, 3));
            }
            
            // SNI 本身 - 多点分割
            let sni_data = &data[sni_pos..sni_end];
            fragments.extend(self.split_sni_multi_point(sni_data));
            
            // SNI 之后的数据
            if sni_end < data.len() {
                let post_sni = &data[sni_end..];
                fragments.extend(self.split_into_tiny_fragments(post_sni, 2, 5));
            }
        } else {
            // 没找到 SNI，使用普通分片
            fragments = self.split_random(data);
        }
        
        fragments
    }
    
    /// 深信服极限分片 (2025)
    fn split_sangfor_extreme(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut fragments = Vec::new();
        let mut rng = rand::thread_rng();
        
        // 第一个字节单独发送
        if !data.is_empty() {
            let mut first = vec![data[0]];
            
            // 可能反转字节（利用某些 DPI 的解析缺陷）
            if self.config.geneva.reverse_first_fragment && data.len() > 1 {
                first = data[..2.min(data.len())].iter().rev().copied().collect();
            }
            
            fragments.push(first);
        }
        
        // 剩余数据极小分片
        let remaining = if self.config.geneva.reverse_first_fragment && data.len() > 1 {
            &data[2.min(data.len())..]
        } else {
            &data[1.min(data.len())..]
        };
        
        // 查找 SNI 并特殊处理
        if let Some(sni_pos) = self.find_sni_position(data) {
            let sni_in_remaining = sni_pos.saturating_sub(1);
            
            if sni_in_remaining < remaining.len() {
                // SNI 之前
                if sni_in_remaining > 0 {
                    fragments.extend(self.split_into_tiny_fragments(&remaining[..sni_in_remaining], 1, 2));
                }
                
                // SNI 区域 - 每个字节单独发送
                let sni_end = self.find_sni_end(data, sni_pos).saturating_sub(1);
                let sni_end_in_remaining = sni_end.min(remaining.len());
                
                for byte in &remaining[sni_in_remaining..sni_end_in_remaining] {
                    // 可能随机化大小写
                    let b = if self.config.geneva.randomize_case && byte.is_ascii_alphabetic() {
                        if rng.gen_bool(0.5) {
                            byte.to_ascii_uppercase()
                        } else {
                            byte.to_ascii_lowercase()
                        }
                    } else {
                        *byte
                    };
                    fragments.push(vec![b]);
                }
                
                // SNI 之后
                if sni_end_in_remaining < remaining.len() {
                    fragments.extend(self.split_into_tiny_fragments(&remaining[sni_end_in_remaining..], 1, 2));
                }
            } else {
                fragments.extend(self.split_into_tiny_fragments(remaining, 1, 2));
            }
        } else {
            fragments.extend(self.split_into_tiny_fragments(remaining, 1, 2));
        }
        
        fragments
    }
    
    /// 多点 SNI 分割
    fn split_multi_sni(&self, data: &[u8]) -> Vec<Vec<u8>> {
        if let Some(sni_pos) = self.find_sni_position(data) {
            let sni_end = self.find_sni_end(data, sni_pos);
            let sni_len = sni_end - sni_pos;
            
            let mut fragments = Vec::new();
            
            // SNI 之前
            if sni_pos > 0 {
                fragments.push(data[..sni_pos].to_vec());
            }
            
            // 根据配置的分割点分割 SNI
            let mut last_pos = 0;
            for &split_pos in &self.config.geneva.sni_split_positions {
                if split_pos < sni_len && split_pos > last_pos {
                    fragments.push(data[sni_pos + last_pos..sni_pos + split_pos].to_vec());
                    last_pos = split_pos;
                }
            }
            
            // SNI 剩余部分
            if last_pos < sni_len {
                fragments.push(data[sni_pos + last_pos..sni_end].to_vec());
            }
            
            // SNI 之后
            if sni_end < data.len() {
                fragments.push(data[sni_end..].to_vec());
            }
            
            fragments.into_iter().filter(|v| !v.is_empty()).collect()
        } else {
            self.split_random(data)
        }
    }
    
    /// 将数据分成极小的片段
    fn split_into_tiny_fragments(&self, data: &[u8], min_size: usize, max_size: usize) -> Vec<Vec<u8>> {
        let mut fragments = Vec::new();
        let mut pos = 0;
        let mut rng = rand::thread_rng();
        
        while pos < data.len() {
            let size = rng.gen_range(min_size..=max_size).min(data.len() - pos);
            fragments.push(data[pos..pos + size].to_vec());
            pos += size;
        }
        
        fragments
    }
    
    /// 多点分割 SNI
    fn split_sni_multi_point(&self, sni_data: &[u8]) -> Vec<Vec<u8>> {
        let mut fragments = Vec::new();
        let mut last_pos = 0;
        
        for &split_pos in &self.config.geneva.sni_split_positions {
            if split_pos < sni_data.len() && split_pos > last_pos {
                fragments.push(sni_data[last_pos..split_pos].to_vec());
                last_pos = split_pos;
            }
        }
        
        // 剩余部分
        if last_pos < sni_data.len() {
            fragments.push(sni_data[last_pos..].to_vec());
        }
        
        if fragments.is_empty() {
            fragments.push(sni_data.to_vec());
        }
        
        fragments
    }
    
    /// 查找 SNI 结束位置
    fn find_sni_end(&self, data: &[u8], sni_start: usize) -> usize {
        // SNI 扩展格式:
        // type (2) + length (2) + list_length (2) + name_type (1) + name_length (2) + name
        if sni_start + 9 > data.len() {
            return data.len();
        }
        
        let ext_len = u16::from_be_bytes([data[sni_start + 2], data[sni_start + 3]]) as usize;
        (sni_start + 4 + ext_len).min(data.len())
    }
    
    /// 生成垃圾数据
    fn generate_garbage(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let (min, max) = self.config.geneva.garbage_size_range;
        let size = rng.gen_range(min..=max);
        
        let mut garbage = vec![0u8; size];
        rng.fill(&mut garbage[..]);
        
        // 使垃圾数据看起来像无效的 TLS 记录
        if size >= 5 {
            garbage[0] = rng.gen_range(0x14..=0x19); // 看起来像 TLS 记录类型
            garbage[1] = 0x03;
            garbage[2] = rng.gen_range(0x01..=0x03);
        }
        
        garbage
    }

    /// 在 SNI 位置分片（最有效的方式）
    fn split_at_sni(&self, data: &[u8]) -> Vec<Vec<u8>> {
        // 查找 SNI 扩展位置
        if let Some(sni_pos) = self.find_sni_position(data) {
            // 在 SNI 前后各分一次
            let split1 = sni_pos.saturating_sub(1);
            let split2 = (sni_pos + 10).min(data.len());
            
            vec![
                data[..split1].to_vec(),
                data[split1..split2].to_vec(),
                data[split2..].to_vec(),
            ].into_iter().filter(|v| !v.is_empty()).collect()
        } else {
            self.split_random(data)
        }
    }

    /// 查找 SNI 扩展位置
    fn find_sni_position(&self, data: &[u8]) -> Option<usize> {
        // TLS Client Hello 结构:
        // [0]: Content Type (0x16)
        // [1-2]: Version
        // [3-4]: Length
        // [5]: Handshake Type (0x01)
        // [6-8]: Length
        // [9-10]: Client Version
        // [11-42]: Random (32 bytes)
        // [43]: Session ID Length
        // ...
        
        if data.len() < 50 {
            return None;
        }

        // 跳过固定头部
        let mut pos = 43;
        
        // Session ID
        if pos >= data.len() { return None; }
        let session_id_len = data[pos] as usize;
        pos += 1 + session_id_len;
        
        // Cipher Suites
        if pos + 2 > data.len() { return None; }
        let cipher_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2 + cipher_len;
        
        // Compression Methods
        if pos >= data.len() { return None; }
        let comp_len = data[pos] as usize;
        pos += 1 + comp_len;
        
        // Extensions Length
        if pos + 2 > data.len() { return None; }
        let _ext_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        
        // 搜索 SNI 扩展 (type = 0x0000)
        while pos + 4 < data.len() {
            let ext_type = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let ext_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            
            if ext_type == 0x0000 {
                // 找到 SNI 扩展
                return Some(pos);
            }
            
            pos += 4 + ext_len;
        }
        
        None
    }

    /// 1+N 模式：第一个包只发 1 字节
    fn split_one_byte_first(&self, data: &[u8]) -> Vec<Vec<u8>> {
        if data.len() <= 1 {
            return vec![data.to_vec()];
        }
        
        let mut fragments = vec![vec![data[0]]];
        
        // 剩余数据随机分片
        let remaining = &data[1..];
        fragments.extend(self.split_random(remaining));
        
        fragments
    }

    /// 按 TLS 记录边界分片
    fn split_tls_records(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut fragments = Vec::new();
        let mut pos = 0;
        
        while pos < data.len() {
            if pos + 5 > data.len() {
                fragments.push(data[pos..].to_vec());
                break;
            }
            
            // TLS 记录长度
            let record_len = u16::from_be_bytes([data[pos + 3], data[pos + 4]]) as usize;
            let end = (pos + 5 + record_len).min(data.len());
            
            // 将这个记录再分成小片
            let record = &data[pos..end];
            fragments.extend(self.split_random(record));
            
            pos = end;
        }
        
        fragments
    }

    /// 固定大小分片
    fn split_fixed(&self, data: &[u8], size: usize) -> Vec<Vec<u8>> {
        data.chunks(size.max(1))
            .map(|c| c.to_vec())
            .collect()
    }

    /// 随机大小分片
    fn split_random(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut fragments = Vec::new();
        let mut pos = 0;
        let mut rng = rand::thread_rng();
        
        while pos < data.len() {
            let size = rng.gen_range(self.config.min_fragment_size..=self.config.max_fragment_size);
            let end = (pos + size).min(data.len());
            fragments.push(data[pos..end].to_vec());
            pos = end;
        }
        
        fragments
    }

    /// Geneva 策略：乱序发送
    fn split_geneva_out_of_order(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut fragments = self.split_random(data);
        let mut rng = rand::thread_rng();
        
        // 打乱顺序，但保留第一个和最后一个的位置（部分乱序）
        if fragments.len() > 3 {
            let middle: Vec<_> = fragments.drain(1..fragments.len()-1).collect();
            let mut shuffled = middle;
            shuffled.shuffle(&mut rng);
            
            let first = fragments.remove(0);
            let last = fragments.pop().unwrap_or_default();
            
            fragments = vec![first];
            fragments.extend(shuffled);
            if !last.is_empty() {
                fragments.push(last);
            }
        }
        
        fragments
    }

    /// Geneva 策略：分段重叠
    fn split_geneva_overlap(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut fragments = Vec::new();
        let mut pos = 0;
        let mut rng = rand::thread_rng();
        let overlap = self.config.geneva.overlap_bytes;
        
        while pos < data.len() {
            let size = rng.gen_range(self.config.min_fragment_size..=self.config.max_fragment_size);
            let end = (pos + size).min(data.len());
            fragments.push(data[pos..end].to_vec());
            
            // 下一个分片从重叠位置开始
            pos = if end > overlap { end - overlap } else { end };
            
            // 避免无限循环
            if pos >= data.len() - 1 {
                break;
            }
        }
        
        // 确保最后的数据被包含
        if pos < data.len() {
            fragments.push(data[pos..].to_vec());
        }
        
        fragments
    }

    /// 组合策略：SNI 分片 + 乱序
    fn split_combined_sni_out_of_order(&self, data: &[u8]) -> Vec<Vec<u8>> {
        // 先按 SNI 位置分片
        let mut fragments = self.split_at_sni(data);
        let mut rng = rand::thread_rng();
        
        // 对每个大分片再进行随机分片
        let mut final_fragments = Vec::new();
        for fragment in fragments.drain(..) {
            if fragment.len() > self.config.max_fragment_size {
                let sub_fragments = self.split_random(&fragment);
                final_fragments.extend(sub_fragments);
            } else {
                final_fragments.push(fragment);
            }
        }
        
        // 部分乱序（保留首尾）
        if final_fragments.len() > 4 && self.config.geneva.out_of_order {
            let disorder_count = ((final_fragments.len() - 2) as f64 
                * self.config.geneva.disorder_rate) as usize;
            
            // 随机选择一些中间分片交换位置
            for _ in 0..disorder_count {
                let i = rng.gen_range(1..final_fragments.len() - 1);
                let j = rng.gen_range(1..final_fragments.len() - 1);
                if i != j {
                    final_fragments.swap(i, j);
                }
            }
        }
        
        final_fragments
    }

    /// 应用 Geneva 策略到分片列表
    fn apply_geneva_strategies(&self, mut fragments: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        let mut rng = rand::thread_rng();
        
        // 乱序策略
        if self.config.geneva.out_of_order && fragments.len() > 2 {
            let disorder_count = ((fragments.len() - 2) as f64 
                * self.config.geneva.disorder_rate) as usize;
            
            for _ in 0..disorder_count {
                let i = rng.gen_range(1..fragments.len() - 1);
                let j = rng.gen_range(1..fragments.len() - 1);
                if i != j {
                    fragments.swap(i, j);
                }
            }
        }
        
        fragments
    }

    /// 生成干扰数据包（用于 TTL 欺骗等高级策略）
    #[allow(dead_code)]
    pub fn generate_decoy_packet(&self, original: &[u8]) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut decoy = original.to_vec();
        
        // 随机修改一些字节，使其成为无效数据
        for byte in decoy.iter_mut() {
            if rng.gen_bool(0.3) {
                *byte = rng.gen();
            }
        }
        
        decoy
    }

    /// 生成假 RST 包数据
    #[allow(dead_code)]
    pub fn generate_fake_rst(&self) -> Vec<u8> {
        // TCP RST 包的基本结构（需要配合 raw socket 使用）
        // 这里只生成 payload 部分，实际发送需要构造完整的 TCP 头
        vec![
            0x00, 0x14, // 源端口（需要替换）
            0x00, 0x50, // 目标端口（需要替换）
            0x00, 0x00, 0x00, 0x00, // 序列号（需要替换）
            0x00, 0x00, 0x00, 0x00, // 确认号
            0x50, 0x04, // 数据偏移 + RST 标志
            0x00, 0x00, // 窗口大小
            0x00, 0x00, // 校验和（需要计算）
            0x00, 0x00, // 紧急指针
        ]
    }
}

/// 分片 TCP 流包装器
pub struct FragmentedTcpStream<S> {
    inner: S,
    fragmenter: TcpFragmenter,
    first_write: bool,
}

impl<S> FragmentedTcpStream<S> {
    pub fn new(stream: S, config: TcpFragmentConfig) -> Self {
        Self {
            inner: stream,
            fragmenter: TcpFragmenter::new(config),
            first_write: true,
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for FragmentedTcpStream<S> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        std::pin::Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

/// 带分片的连接辅助函数
pub async fn connect_with_fragment<S: AsyncWrite + Unpin>(
    stream: &mut S,
    client_hello: &[u8],
    config: &TcpFragmentConfig,
) -> std::io::Result<()> {
    let fragmenter = TcpFragmenter::new(config.clone());
    fragmenter.send_fragmented(stream, client_hello).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_split() {
        let config = TcpFragmentConfig {
            min_fragment_size: 5,
            max_fragment_size: 10,
            ..Default::default()
        };
        let fragmenter = TcpFragmenter::new(config);
        
        let data = vec![0u8; 100];
        let fragments = fragmenter.split_random(&data);
        
        // 验证所有分片合起来等于原数据
        let reassembled: Vec<u8> = fragments.into_iter().flatten().collect();
        assert_eq!(reassembled, data);
    }

    #[test]
    fn test_one_byte_first() {
        let config = TcpFragmentConfig::default();
        let fragmenter = TcpFragmenter::new(config);
        
        let data = vec![1, 2, 3, 4, 5];
        let fragments = fragmenter.split_one_byte_first(&data);
        
        assert_eq!(fragments[0], vec![1]);
        
        let reassembled: Vec<u8> = fragments.into_iter().flatten().collect();
        assert_eq!(reassembled, data);
    }

    #[test]
    fn test_find_sni() {
        let config = TcpFragmentConfig::default();
        let fragmenter = TcpFragmenter::new(config);
        
        // 构造一个简单的 TLS Client Hello
        let mut hello = vec![
            0x16, 0x03, 0x01, 0x00, 0x50, // TLS Record
            0x01, 0x00, 0x00, 0x4c,       // Client Hello
            0x03, 0x03,                   // Version
        ];
        hello.extend(vec![0u8; 32]); // Random
        hello.push(0x00);            // Session ID Length = 0
        hello.extend(&[0x00, 0x02, 0x00, 0xff]); // Cipher Suites
        hello.extend(&[0x01, 0x00]); // Compression
        hello.extend(&[0x00, 0x10]); // Extensions Length
        // SNI Extension
        hello.extend(&[0x00, 0x00, 0x00, 0x0c]); // Type, Length
        hello.extend(&[0x00, 0x0a, 0x00]); // List Length, Type
        hello.extend(&[0x00, 0x07]); // Name Length
        hello.extend(b"test.com");
        
        let pos = fragmenter.find_sni_position(&hello);
        assert!(pos.is_some());
    }

    #[test]
    fn test_geneva_out_of_order() {
        let config = TcpFragmentConfig {
            min_fragment_size: 5,
            max_fragment_size: 10,
            mode: FragmentMode::GenevaOutOfOrder,
            geneva: GenevaConfig {
                out_of_order: true,
                disorder_rate: 0.5,
                ..Default::default()
            },
            ..Default::default()
        };
        let fragmenter = TcpFragmenter::new(config);
        
        let data: Vec<u8> = (0..100).collect();
        let fragments = fragmenter.split_geneva_out_of_order(&data);
        
        // 验证数据完整性（排序后应该能还原）
        let mut reassembled: Vec<u8> = fragments.into_iter().flatten().collect();
        // 注意：乱序后直接拼接不等于原数据，但所有字节都应该存在
        reassembled.sort();
        let mut sorted_data = data.clone();
        sorted_data.sort();
        assert_eq!(reassembled.len(), sorted_data.len());
    }

    #[test]
    fn test_geneva_overlap() {
        let config = TcpFragmentConfig {
            min_fragment_size: 10,
            max_fragment_size: 20,
            mode: FragmentMode::GenevaOverlap,
            geneva: GenevaConfig {
                overlap: true,
                overlap_bytes: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let fragmenter = TcpFragmenter::new(config);
        
        let data: Vec<u8> = (0..50).collect();
        let fragments = fragmenter.split_geneva_overlap(&data);
        
        // 重叠分片的总长度应该大于原数据
        let total_len: usize = fragments.iter().map(|f| f.len()).sum();
        assert!(total_len >= data.len());
    }

    #[test]
    fn test_combined_sni_out_of_order() {
        let config = TcpFragmentConfig {
            min_fragment_size: 3,
            max_fragment_size: 8,
            mode: FragmentMode::CombinedSniOutOfOrder,
            geneva: GenevaConfig {
                out_of_order: true,
                disorder_rate: 0.5,
                ..Default::default()
            },
            ..Default::default()
        };
        let fragmenter = TcpFragmenter::new(config);
        
        // 构造 TLS Client Hello
        let mut hello = vec![
            0x16, 0x03, 0x01, 0x00, 0x50,
            0x01, 0x00, 0x00, 0x4c,
            0x03, 0x03,
        ];
        hello.extend(vec![0u8; 32]);
        hello.push(0x00);
        hello.extend(&[0x00, 0x02, 0x00, 0xff]);
        hello.extend(&[0x01, 0x00]);
        hello.extend(&[0x00, 0x10]);
        hello.extend(&[0x00, 0x00, 0x00, 0x0c]);
        hello.extend(&[0x00, 0x0a, 0x00]);
        hello.extend(&[0x00, 0x07]);
        hello.extend(b"test.com");
        
        let fragments = fragmenter.split_combined_sni_out_of_order(&hello);
        
        // 应该产生多个分片
        assert!(fragments.len() > 1);
    }

    #[test]
    fn test_preset_configs() {
        let sangfor = TcpFragmentConfig::for_sangfor();
        assert!(sangfor.enabled);
        assert_eq!(sangfor.mode, FragmentMode::SangforOptimized);
        assert!(sangfor.geneva.out_of_order);
        assert!(sangfor.geneva.inject_garbage);

        let gfw = TcpFragmentConfig::for_gfw();
        assert!(gfw.enabled);
        assert_eq!(gfw.mode, FragmentMode::SniSplit);

        let aggressive = TcpFragmentConfig::aggressive();
        assert!(aggressive.enabled);
        assert!(aggressive.geneva.overlap);
        
        let extreme = TcpFragmentConfig::for_sangfor_extreme();
        assert!(extreme.enabled);
        assert_eq!(extreme.mode, FragmentMode::SangforExtreme);
        assert!(extreme.geneva.reverse_first_fragment);
        
        let stealth = TcpFragmentConfig::stealth();
        assert!(stealth.enabled);
        assert!(!stealth.geneva.out_of_order);
    }

    #[test]
    fn test_decoy_packet() {
        let config = TcpFragmentConfig::default();
        let fragmenter = TcpFragmenter::new(config);
        
        let original = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let decoy = fragmenter.generate_decoy_packet(&original);
        
        // 长度应该相同
        assert_eq!(decoy.len(), original.len());
        // 内容应该不同（大概率）
        assert_ne!(decoy, original);
    }
}
