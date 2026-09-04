//! WebRTC 隧道模块 - 将 VPN 流量伪装成视频会议流量
//!
//! 原理：
//! 1. 使用 WebRTC DataChannel 传输数据
//! 2. 流量特征与 Zoom/Teams/Google Meet 一致
//! 3. 使用 DTLS 加密，与正常 WebRTC 无法区分
//! 4. 支持 STUN/TURN 穿透
//!
//! 特点：
//! - 完美模拟视频会议流量
//! - 支持 NAT 穿透
//! - 使用 UDP，低延迟
//! - 深信服难以阻断（会影响正常视频会议）

use std::net::SocketAddr;
use rand::Rng;

/// WebRTC 隧道配置
#[derive(Clone, Debug)]
pub struct WebRtcTunnelConfig {
    /// 是否启用
    pub enabled: bool,
    /// STUN 服务器列表
    pub stun_servers: Vec<String>,
    /// TURN 服务器
    pub turn_server: Option<TurnConfig>,
    /// 是否模拟视频流
    pub simulate_video: bool,
    /// 模拟的视频比特率 (kbps)
    pub video_bitrate: u32,
    /// 是否模拟音频流
    pub simulate_audio: bool,
    /// 模拟的音频比特率 (kbps)
    pub audio_bitrate: u32,
    /// DataChannel 标签
    pub data_channel_label: String,
}

impl Default for WebRtcTunnelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            stun_servers: vec![
                "stun:stun.l.google.com:19302".to_string(),
                "stun:stun1.l.google.com:19302".to_string(),
                "stun:stun2.l.google.com:19302".to_string(),
            ],
            turn_server: None,
            simulate_video: true,
            video_bitrate: 2500,
            simulate_audio: true,
            audio_bitrate: 128,
            data_channel_label: "data".to_string(),
        }
    }
}

/// TURN 服务器配置
#[derive(Clone, Debug)]
pub struct TurnConfig {
    pub url: String,
    pub username: String,
    pub credential: String,
}

/// STUN 消息类型
#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StunMessageType {
    BindingRequest = 0x0001,
    BindingResponse = 0x0101,
    BindingErrorResponse = 0x0111,
}

/// STUN 属性类型
#[repr(u16)]
#[derive(Clone, Copy, Debug)]
pub enum StunAttributeType {
    MappedAddress = 0x0001,
    Username = 0x0006,
    MessageIntegrity = 0x0008,
    ErrorCode = 0x0009,
    UnknownAttributes = 0x000A,
    Realm = 0x0014,
    Nonce = 0x0015,
    XorMappedAddress = 0x0020,
    Software = 0x8022,
    Fingerprint = 0x8028,
}

/// STUN 消息
#[derive(Clone, Debug)]
pub struct StunMessage {
    pub message_type: StunMessageType,
    pub transaction_id: [u8; 12],
    pub attributes: Vec<StunAttribute>,
}

/// STUN 属性
#[derive(Clone, Debug)]
pub struct StunAttribute {
    pub attr_type: u16,
    pub value: Vec<u8>,
}

impl StunMessage {
    /// 创建 Binding Request
    pub fn binding_request() -> Self {
        let mut transaction_id = [0u8; 12];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut transaction_id);
        
        Self {
            message_type: StunMessageType::BindingRequest,
            transaction_id,
            attributes: Vec::new(),
        }
    }

    /// 编码 STUN 消息
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        
        // Message Type (2 bytes)
        buf.extend_from_slice(&(self.message_type as u16).to_be_bytes());
        
        // Message Length (2 bytes) - 先占位
        let length_pos = buf.len();
        buf.extend_from_slice(&[0, 0]);
        
        // Magic Cookie (4 bytes)
        buf.extend_from_slice(&0x2112A442u32.to_be_bytes());
        
        // Transaction ID (12 bytes)
        buf.extend_from_slice(&self.transaction_id);
        
        // Attributes
        for attr in &self.attributes {
            buf.extend_from_slice(&attr.attr_type.to_be_bytes());
            buf.extend_from_slice(&(attr.value.len() as u16).to_be_bytes());
            buf.extend_from_slice(&attr.value);
            
            // 4 字节对齐
            let padding = (4 - (attr.value.len() % 4)) % 4;
            buf.extend(vec![0u8; padding]);
        }
        
        // 更新长度
        let length = (buf.len() - 20) as u16;
        buf[length_pos..length_pos + 2].copy_from_slice(&length.to_be_bytes());
        
        buf
    }

    /// 解码 STUN 消息
    pub fn decode(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 20 {
            return Err("消息太短");
        }
        
        let message_type = u16::from_be_bytes([data[0], data[1]]);
        let message_type = match message_type {
            0x0001 => StunMessageType::BindingRequest,
            0x0101 => StunMessageType::BindingResponse,
            0x0111 => StunMessageType::BindingErrorResponse,
            _ => return Err("未知消息类型"),
        };
        
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;
        let magic = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        
        if magic != 0x2112A442 {
            return Err("Magic Cookie 不匹配");
        }
        
        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&data[8..20]);
        
        let mut attributes = Vec::new();
        let mut pos = 20;
        
        while pos < 20 + length && pos + 4 <= data.len() {
            let attr_type = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let attr_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            pos += 4;
            
            if pos + attr_len > data.len() {
                break;
            }
            
            attributes.push(StunAttribute {
                attr_type,
                value: data[pos..pos + attr_len].to_vec(),
            });
            
            pos += attr_len;
            pos += (4 - (attr_len % 4)) % 4; // 对齐
        }
        
        Ok(Self {
            message_type,
            transaction_id,
            attributes,
        })
    }

    /// 从响应中提取映射地址
    pub fn get_mapped_address(&self) -> Option<SocketAddr> {
        for attr in &self.attributes {
            if attr.attr_type == StunAttributeType::XorMappedAddress as u16 
                || attr.attr_type == StunAttributeType::MappedAddress as u16 
            {
                if attr.value.len() >= 8 {
                    let family = attr.value[1];
                    let port = u16::from_be_bytes([attr.value[2], attr.value[3]]);
                    
                    // XOR 解码
                    let xor_port = if attr.attr_type == StunAttributeType::XorMappedAddress as u16 {
                        port ^ 0x2112
                    } else {
                        port
                    };
                    
                    if family == 0x01 && attr.value.len() >= 8 {
                        // IPv4
                        let mut ip = [attr.value[4], attr.value[5], attr.value[6], attr.value[7]];
                        if attr.attr_type == StunAttributeType::XorMappedAddress as u16 {
                            ip[0] ^= 0x21;
                            ip[1] ^= 0x12;
                            ip[2] ^= 0xA4;
                            ip[3] ^= 0x42;
                        }
                        return Some(SocketAddr::from((ip, xor_port)));
                    }
                }
            }
        }
        None
    }
}

/// RTP 包头
#[derive(Clone, Debug)]
pub struct RtpHeader {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub csrc_count: u8,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
}

impl RtpHeader {
    /// 创建视频 RTP 头
    pub fn video(seq: u16, timestamp: u32, ssrc: u32) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: false,
            payload_type: 96, // 动态类型，通常用于 H.264
            sequence_number: seq,
            timestamp,
            ssrc,
        }
    }

    /// 创建音频 RTP 头
    pub fn audio(seq: u16, timestamp: u32, ssrc: u32) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: false,
            payload_type: 111, // Opus
            sequence_number: seq,
            timestamp,
            ssrc,
        }
    }

    /// 编码 RTP 头
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12);
        
        let byte0 = (self.version << 6)
            | ((self.padding as u8) << 5)
            | ((self.extension as u8) << 4)
            | self.csrc_count;
        buf.push(byte0);
        
        let byte1 = ((self.marker as u8) << 7) | self.payload_type;
        buf.push(byte1);
        
        buf.extend_from_slice(&self.sequence_number.to_be_bytes());
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.ssrc.to_be_bytes());
        
        buf
    }
}

/// WebRTC 流量模拟器
pub struct WebRtcSimulator {
    config: WebRtcTunnelConfig,
    video_seq: u16,
    audio_seq: u16,
    video_timestamp: u32,
    audio_timestamp: u32,
    video_ssrc: u32,
    audio_ssrc: u32,
}

impl WebRtcSimulator {
    pub fn new(config: WebRtcTunnelConfig) -> Self {
        let mut rng = rand::thread_rng();
        Self {
            config,
            video_seq: rng.gen(),
            audio_seq: rng.gen(),
            video_timestamp: rng.gen(),
            audio_timestamp: rng.gen(),
            video_ssrc: rng.gen(),
            audio_ssrc: rng.gen(),
        }
    }

    /// 将数据封装为 RTP 包（模拟视频）
    pub fn wrap_as_video(&mut self, data: &[u8]) -> Vec<u8> {
        let header = RtpHeader::video(self.video_seq, self.video_timestamp, self.video_ssrc);
        self.video_seq = self.video_seq.wrapping_add(1);
        self.video_timestamp = self.video_timestamp.wrapping_add(3000); // 30fps
        
        let mut packet = header.encode();
        packet.extend_from_slice(data);
        packet
    }

    /// 将数据封装为 RTP 包（模拟音频）
    pub fn wrap_as_audio(&mut self, data: &[u8]) -> Vec<u8> {
        let header = RtpHeader::audio(self.audio_seq, self.audio_timestamp, self.audio_ssrc);
        self.audio_seq = self.audio_seq.wrapping_add(1);
        self.audio_timestamp = self.audio_timestamp.wrapping_add(960); // 48kHz, 20ms
        
        let mut packet = header.encode();
        packet.extend_from_slice(data);
        packet
    }

    /// 生成虚假的视频帧
    pub fn generate_fake_video_frame(&mut self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        
        // 模拟 H.264 NAL 单元
        let frame_size = (self.config.video_bitrate as usize * 1000 / 8 / 30)
            .saturating_add(rng.gen_range(0..500));
        
        let mut frame = vec![0u8; frame_size.min(1400)]; // MTU 限制
        rng.fill(&mut frame[..]);
        
        // 添加 H.264 NAL 头
        frame[0] = 0x00;
        frame[1] = 0x00;
        frame[2] = 0x00;
        frame[3] = 0x01;
        frame[4] = 0x41; // Non-IDR slice
        
        self.wrap_as_video(&frame)
    }

    /// 生成虚假的音频帧
    pub fn generate_fake_audio_frame(&mut self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        
        // 模拟 Opus 帧 (20ms @ 128kbps ≈ 320 bytes)
        let frame_size = (self.config.audio_bitrate as usize * 1000 / 8 / 50)
            .saturating_add(rng.gen_range(0..50));
        
        let mut frame = vec![0u8; frame_size.min(320)];
        rng.fill(&mut frame[..]);
        
        self.wrap_as_audio(&frame)
    }

    /// 从 RTP 包中提取数据
    pub fn unwrap_rtp(packet: &[u8]) -> Option<Vec<u8>> {
        if packet.len() < 12 {
            return None;
        }
        
        let version = (packet[0] >> 6) & 0x03;
        if version != 2 {
            return None;
        }
        
        let csrc_count = packet[0] & 0x0f;
        let extension = (packet[0] >> 4) & 0x01;
        
        let mut header_len = 12 + (csrc_count as usize * 4);
        
        if extension == 1 && packet.len() > header_len + 4 {
            let ext_len = u16::from_be_bytes([
                packet[header_len + 2],
                packet[header_len + 3],
            ]) as usize * 4;
            header_len += 4 + ext_len;
        }
        
        if packet.len() > header_len {
            Some(packet[header_len..].to_vec())
        } else {
            None
        }
    }
}

/// DTLS 指纹
pub struct DtlsFingerprint {
    pub algorithm: String,
    pub value: String,
}

impl DtlsFingerprint {
    /// 生成随机指纹（用于 SDP）
    pub fn generate() -> Self {
        let mut hash = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut hash);
        
        let value = hash
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(":");
        
        Self {
            algorithm: "sha-256".to_string(),
            value,
        }
    }
}

/// 生成 SDP Offer（简化版）
pub fn generate_sdp_offer(fingerprint: &DtlsFingerprint, ice_ufrag: &str, ice_pwd: &str) -> String {
    format!(
        "v=0\r\n\
         o=- {} 2 IN IP4 127.0.0.1\r\n\
         s=-\r\n\
         t=0 0\r\n\
         a=group:BUNDLE 0\r\n\
         a=msid-semantic: WMS\r\n\
         m=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n\
         c=IN IP4 0.0.0.0\r\n\
         a=ice-ufrag:{}\r\n\
         a=ice-pwd:{}\r\n\
         a=ice-options:trickle\r\n\
         a=fingerprint:{} {}\r\n\
         a=setup:actpass\r\n\
         a=mid:0\r\n\
         a=sctp-port:5000\r\n\
         a=max-message-size:262144\r\n",
        rand::thread_rng().gen::<u64>(),
        ice_ufrag,
        ice_pwd,
        fingerprint.algorithm,
        fingerprint.value
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stun_binding_request() {
        let msg = StunMessage::binding_request();
        let encoded = msg.encode();
        
        assert!(encoded.len() >= 20);
        assert_eq!(encoded[0], 0x00);
        assert_eq!(encoded[1], 0x01);
        
        let decoded = StunMessage::decode(&encoded).unwrap();
        assert_eq!(decoded.message_type, StunMessageType::BindingRequest);
        assert_eq!(decoded.transaction_id, msg.transaction_id);
    }

    #[test]
    fn test_rtp_header() {
        let header = RtpHeader::video(1234, 5678, 0xDEADBEEF);
        let encoded = header.encode();
        
        assert_eq!(encoded.len(), 12);
        assert_eq!(encoded[0] >> 6, 2); // version
        assert_eq!(encoded[1] & 0x7f, 96); // payload type
    }

    #[test]
    fn test_webrtc_simulator() {
        let config = WebRtcTunnelConfig::default();
        let mut sim = WebRtcSimulator::new(config);
        
        let data = b"test data";
        let video_packet = sim.wrap_as_video(data);
        
        assert!(video_packet.len() > 12);
        
        let unwrapped = WebRtcSimulator::unwrap_rtp(&video_packet).unwrap();
        assert_eq!(unwrapped, data);
    }

    #[test]
    fn test_dtls_fingerprint() {
        let fp = DtlsFingerprint::generate();
        assert_eq!(fp.algorithm, "sha-256");
        assert!(fp.value.contains(':'));
    }
}
