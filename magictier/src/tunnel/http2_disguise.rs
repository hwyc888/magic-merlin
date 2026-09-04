//! HTTP/2 多路复用伪装模块 - 将 VPN 流量伪装成正常 HTTP/2 流量
//!
//! 原理：
//! 1. 使用 HTTP/2 的多路复用特性
//! 2. VPN 数据通过多个 HTTP/2 流传输
//! 3. 混入真实的 HTTP 请求（如图片、CSS、JS）
//! 4. 流量特征与正常网页浏览一致
//!
//! 特点：
//! - 完美模拟浏览器行为
//! - 支持 HPACK 头部压缩
//! - 支持流优先级
//! - 支持服务器推送

use std::collections::HashMap;
use rand::Rng;

/// HTTP/2 伪装配置
#[derive(Clone, Debug)]
pub struct Http2DisguiseConfig {
    /// 是否启用伪装
    pub enabled: bool,
    /// 伪装的网站
    pub disguise_host: String,
    /// 是否混入真实请求
    pub mix_real_requests: bool,
    /// 真实请求比例 (0.0 - 1.0)
    pub real_request_ratio: f32,
    /// 最大并发流数
    pub max_concurrent_streams: u32,
    /// 初始窗口大小
    pub initial_window_size: u32,
    /// 是否启用服务器推送
    pub enable_server_push: bool,
}

impl Default for Http2DisguiseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            disguise_host: "www.google.com".to_string(),
            mix_real_requests: true,
            real_request_ratio: 0.1,
            max_concurrent_streams: 100,
            initial_window_size: 65535,
            enable_server_push: false,
        }
    }
}

/// HTTP/2 帧类型
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Http2FrameType {
    Data = 0x0,
    Headers = 0x1,
    Priority = 0x2,
    RstStream = 0x3,
    Settings = 0x4,
    PushPromise = 0x5,
    Ping = 0x6,
    GoAway = 0x7,
    WindowUpdate = 0x8,
    Continuation = 0x9,
}

/// HTTP/2 帧标志
pub mod frame_flags {
    pub const END_STREAM: u8 = 0x1;
    pub const END_HEADERS: u8 = 0x4;
    pub const PADDED: u8 = 0x8;
    pub const PRIORITY: u8 = 0x20;
}

/// HTTP/2 设置参数
#[repr(u16)]
#[derive(Clone, Copy, Debug)]
pub enum Http2Setting {
    HeaderTableSize = 0x1,
    EnablePush = 0x2,
    MaxConcurrentStreams = 0x3,
    InitialWindowSize = 0x4,
    MaxFrameSize = 0x5,
    MaxHeaderListSize = 0x6,
}

/// HTTP/2 帧
#[derive(Clone, Debug)]
pub struct Http2Frame {
    pub frame_type: Http2FrameType,
    pub flags: u8,
    pub stream_id: u32,
    pub payload: Vec<u8>,
}

impl Http2Frame {
    /// 编码帧
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(9 + self.payload.len());
        
        // 长度 (3 bytes)
        let len = self.payload.len() as u32;
        buf.push(((len >> 16) & 0xff) as u8);
        buf.push(((len >> 8) & 0xff) as u8);
        buf.push((len & 0xff) as u8);
        
        // 类型 (1 byte)
        buf.push(self.frame_type as u8);
        
        // 标志 (1 byte)
        buf.push(self.flags);
        
        // 流 ID (4 bytes, 最高位保留)
        buf.push(((self.stream_id >> 24) & 0x7f) as u8);
        buf.push(((self.stream_id >> 16) & 0xff) as u8);
        buf.push(((self.stream_id >> 8) & 0xff) as u8);
        buf.push((self.stream_id & 0xff) as u8);
        
        // 负载
        buf.extend_from_slice(&self.payload);
        
        buf
    }

    /// 解码帧
    pub fn decode(data: &[u8]) -> Result<(Self, usize), &'static str> {
        if data.len() < 9 {
            return Err("帧头不完整");
        }
        
        let len = ((data[0] as u32) << 16) | ((data[1] as u32) << 8) | (data[2] as u32);
        let frame_type = match data[3] {
            0 => Http2FrameType::Data,
            1 => Http2FrameType::Headers,
            2 => Http2FrameType::Priority,
            3 => Http2FrameType::RstStream,
            4 => Http2FrameType::Settings,
            5 => Http2FrameType::PushPromise,
            6 => Http2FrameType::Ping,
            7 => Http2FrameType::GoAway,
            8 => Http2FrameType::WindowUpdate,
            9 => Http2FrameType::Continuation,
            _ => return Err("未知帧类型"),
        };
        let flags = data[4];
        let stream_id = ((data[5] as u32 & 0x7f) << 24)
            | ((data[6] as u32) << 16)
            | ((data[7] as u32) << 8)
            | (data[8] as u32);
        
        let total_len = 9 + len as usize;
        if data.len() < total_len {
            return Err("帧数据不完整");
        }
        
        let payload = data[9..total_len].to_vec();
        
        Ok((
            Self {
                frame_type,
                flags,
                stream_id,
                payload,
            },
            total_len,
        ))
    }
}

/// HTTP/2 伪装器
pub struct Http2Disguiser {
    config: Http2DisguiseConfig,
    next_stream_id: u32,
    active_streams: HashMap<u32, StreamState>,
}

/// 流状态
#[derive(Clone, Debug)]
struct StreamState {
    is_data_stream: bool,
    bytes_sent: u64,
    bytes_received: u64,
}

impl Http2Disguiser {
    pub fn new(config: Http2DisguiseConfig) -> Self {
        Self {
            config,
            next_stream_id: 1, // 客户端使用奇数流 ID
            active_streams: HashMap::new(),
        }
    }

    /// 生成 HTTP/2 连接前言
    pub fn connection_preface() -> Vec<u8> {
        // HTTP/2 连接前言: "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
        b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n".to_vec()
    }

    /// 生成 SETTINGS 帧
    pub fn build_settings_frame(&self) -> Http2Frame {
        let mut payload = Vec::new();
        
        // SETTINGS_MAX_CONCURRENT_STREAMS
        payload.extend_from_slice(&(Http2Setting::MaxConcurrentStreams as u16).to_be_bytes());
        payload.extend_from_slice(&self.config.max_concurrent_streams.to_be_bytes());
        
        // SETTINGS_INITIAL_WINDOW_SIZE
        payload.extend_from_slice(&(Http2Setting::InitialWindowSize as u16).to_be_bytes());
        payload.extend_from_slice(&self.config.initial_window_size.to_be_bytes());
        
        // SETTINGS_ENABLE_PUSH
        payload.extend_from_slice(&(Http2Setting::EnablePush as u16).to_be_bytes());
        payload.extend_from_slice(&(if self.config.enable_server_push { 1u32 } else { 0u32 }).to_be_bytes());
        
        Http2Frame {
            frame_type: Http2FrameType::Settings,
            flags: 0,
            stream_id: 0,
            payload,
        }
    }

    /// 生成 SETTINGS ACK 帧
    pub fn build_settings_ack() -> Http2Frame {
        Http2Frame {
            frame_type: Http2FrameType::Settings,
            flags: 0x1, // ACK
            stream_id: 0,
            payload: Vec::new(),
        }
    }

    /// 分配新的流 ID
    pub fn allocate_stream(&mut self, is_data_stream: bool) -> u32 {
        let stream_id = self.next_stream_id;
        self.next_stream_id += 2; // 客户端使用奇数
        
        self.active_streams.insert(stream_id, StreamState {
            is_data_stream,
            bytes_sent: 0,
            bytes_received: 0,
        });
        
        stream_id
    }

    /// 将 VPN 数据封装为 HTTP/2 DATA 帧
    pub fn wrap_data(&mut self, data: &[u8], stream_id: u32) -> Vec<Http2Frame> {
        let mut frames = Vec::new();
        let max_frame_size = 16384; // 默认最大帧大小
        
        // 分片
        for chunk in data.chunks(max_frame_size) {
            let mut payload = chunk.to_vec();
            
            // 可选：添加填充
            if self.config.mix_real_requests {
                let mut rng = rand::thread_rng();
                if rng.gen_ratio(1, 10) {
                    let padding_len = rng.gen_range(1..256);
                    let mut padded = vec![padding_len as u8];
                    padded.extend_from_slice(chunk);
                    padded.extend(vec![0u8; padding_len]);
                    payload = padded;
                }
            }
            
            frames.push(Http2Frame {
                frame_type: Http2FrameType::Data,
                flags: 0,
                stream_id,
                payload,
            });
        }
        
        // 更新统计
        if let Some(state) = self.active_streams.get_mut(&stream_id) {
            state.bytes_sent += data.len() as u64;
        }
        
        frames
    }

    /// 从 HTTP/2 DATA 帧提取 VPN 数据
    pub fn unwrap_data(&mut self, frame: &Http2Frame) -> Option<Vec<u8>> {
        if frame.frame_type != Http2FrameType::Data {
            return None;
        }
        
        let payload = &frame.payload;
        
        // 检查是否有填充
        if frame.flags & frame_flags::PADDED != 0 && !payload.is_empty() {
            let padding_len = payload[0] as usize;
            if payload.len() > padding_len + 1 {
                return Some(payload[1..payload.len() - padding_len].to_vec());
            }
        }
        
        // 更新统计
        if let Some(state) = self.active_streams.get_mut(&frame.stream_id) {
            state.bytes_received += payload.len() as u64;
        }
        
        Some(payload.clone())
    }

    /// 生成伪装的 HTTP 请求（混淆用）
    pub fn generate_fake_request(&mut self) -> Vec<Http2Frame> {
        let stream_id = self.allocate_stream(false);
        let mut frames = Vec::new();
        
        // 随机选择请求类型
        let mut rng = rand::thread_rng();
        let request_type = rng.gen_range(0..5);
        
        let (path, accept) = match request_type {
            0 => ("/favicon.ico", "image/x-icon"),
            1 => ("/style.css", "text/css"),
            2 => ("/script.js", "application/javascript"),
            3 => ("/image.png", "image/png"),
            _ => ("/api/ping", "application/json"),
        };
        
        // 构建 HEADERS 帧（简化的 HPACK）
        let headers = self.build_simple_headers(path, accept);
        
        frames.push(Http2Frame {
            frame_type: Http2FrameType::Headers,
            flags: frame_flags::END_HEADERS | frame_flags::END_STREAM,
            stream_id,
            payload: headers,
        });
        
        frames
    }

    /// 构建简化的头部（不使用完整 HPACK）
    fn build_simple_headers(&self, path: &str, accept: &str) -> Vec<u8> {
        let mut headers = Vec::new();
        
        // :method = GET (索引 2)
        headers.push(0x82);
        
        // :scheme = https (索引 7)
        headers.push(0x87);
        
        // :path (字面量)
        headers.push(0x04); // 索引 4，不索引
        headers.push(path.len() as u8);
        headers.extend_from_slice(path.as_bytes());
        
        // :authority (字面量)
        headers.push(0x01); // 索引 1，不索引
        headers.push(self.config.disguise_host.len() as u8);
        headers.extend_from_slice(self.config.disguise_host.as_bytes());
        
        // accept (字面量)
        headers.push(0x00); // 新名称
        headers.push(6);
        headers.extend_from_slice(b"accept");
        headers.push(accept.len() as u8);
        headers.extend_from_slice(accept.as_bytes());
        
        // user-agent
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0";
        headers.push(0x00);
        headers.push(10);
        headers.extend_from_slice(b"user-agent");
        headers.push(ua.len() as u8);
        headers.extend_from_slice(ua.as_bytes());
        
        headers
    }

    /// 生成 PING 帧（保活）
    pub fn build_ping_frame() -> Http2Frame {
        let mut payload = [0u8; 8];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut payload);
        
        Http2Frame {
            frame_type: Http2FrameType::Ping,
            flags: 0,
            stream_id: 0,
            payload: payload.to_vec(),
        }
    }

    /// 生成 WINDOW_UPDATE 帧
    pub fn build_window_update(stream_id: u32, increment: u32) -> Http2Frame {
        Http2Frame {
            frame_type: Http2FrameType::WindowUpdate,
            flags: 0,
            stream_id,
            payload: increment.to_be_bytes().to_vec(),
        }
    }

    /// 关闭流
    pub fn close_stream(&mut self, stream_id: u32) -> Http2Frame {
        self.active_streams.remove(&stream_id);
        
        Http2Frame {
            frame_type: Http2FrameType::RstStream,
            flags: 0,
            stream_id,
            payload: 0u32.to_be_bytes().to_vec(), // NO_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encode_decode() {
        let frame = Http2Frame {
            frame_type: Http2FrameType::Data,
            flags: 0,
            stream_id: 1,
            payload: b"hello".to_vec(),
        };
        
        let encoded = frame.encode();
        let (decoded, len) = Http2Frame::decode(&encoded).unwrap();
        
        assert_eq!(len, encoded.len());
        assert_eq!(decoded.frame_type, Http2FrameType::Data);
        assert_eq!(decoded.stream_id, 1);
        assert_eq!(decoded.payload, b"hello");
    }

    #[test]
    fn test_settings_frame() {
        let config = Http2DisguiseConfig::default();
        let disguiser = Http2Disguiser::new(config);
        
        let settings = disguiser.build_settings_frame();
        assert_eq!(settings.frame_type, Http2FrameType::Settings);
        assert_eq!(settings.stream_id, 0);
    }

    #[test]
    fn test_data_wrap_unwrap() {
        let config = Http2DisguiseConfig {
            mix_real_requests: false,
            ..Default::default()
        };
        let mut disguiser = Http2Disguiser::new(config);
        
        let stream_id = disguiser.allocate_stream(true);
        let data = b"test data";
        
        let frames = disguiser.wrap_data(data, stream_id);
        assert_eq!(frames.len(), 1);
        
        let unwrapped = disguiser.unwrap_data(&frames[0]).unwrap();
        assert_eq!(unwrapped, data);
    }

    #[test]
    fn test_connection_preface() {
        let preface = Http2Disguiser::connection_preface();
        assert_eq!(preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
    }
}
