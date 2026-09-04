//! gRPC 伪装模块 - 将流量伪装成 gRPC API 调用
//!
//! 原理：
//! 1. 使用 HTTP/2 + gRPC 协议封装数据
//! 2. 流量看起来像正常的 API 调用
//! 3. 支持 gRPC-Web 模式（HTTP/1.1 兼容）
//! 4. 可以通过任何支持 gRPC 的负载均衡器
//!
//! 深信服很难阻断 gRPC，因为很多企业应用都使用它

use std::collections::HashMap;
use rand::Rng;

/// gRPC 伪装配置
#[derive(Clone, Debug)]
pub struct GrpcDisguiseConfig {
    /// 是否启用
    pub enabled: bool,
    /// 服务名称
    pub service_name: String,
    /// 方法名称
    pub method_name: String,
    /// 伪装的包名
    pub package_name: String,
    /// 是否使用 gRPC-Web
    pub grpc_web: bool,
    /// 自定义元数据
    pub metadata: HashMap<String, String>,
    /// 伪装模式
    pub disguise_mode: GrpcDisguiseMode,
}

impl Default for GrpcDisguiseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service_name: "TunnelService".to_string(),
            method_name: "Tunnel".to_string(),
            package_name: "grpc.tunnel.v1".to_string(),
            grpc_web: false,
            metadata: HashMap::new(),
            disguise_mode: GrpcDisguiseMode::GoogleCloud,
        }
    }
}

/// gRPC 伪装模式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GrpcDisguiseMode {
    /// Google Cloud API 风格
    GoogleCloud,
    /// AWS API 风格
    AwsApi,
    /// Azure API 风格
    AzureApi,
    /// Kubernetes API 风格
    KubernetesApi,
    /// 通用 gRPC
    Generic,
}

impl GrpcDisguiseMode {
    /// 获取对应的服务名称
    pub fn service_name(&self) -> &'static str {
        match self {
            Self::GoogleCloud => "google.cloud.compute.v1.Instances",
            Self::AwsApi => "aws.ec2.DescribeInstances",
            Self::AzureApi => "azure.compute.VirtualMachines",
            Self::KubernetesApi => "k8s.io.api.core.v1.Pod",
            Self::Generic => "grpc.tunnel.v1.TunnelService",
        }
    }

    /// 获取对应的方法名称
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::GoogleCloud => "StreamingCall",
            Self::AwsApi => "StreamData",
            Self::AzureApi => "BidirectionalStream",
            Self::KubernetesApi => "Watch",
            Self::Generic => "Tunnel",
        }
    }
}

/// gRPC 帧类型
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GrpcFrameType {
    /// 数据帧
    Data = 0,
    /// 压缩数据帧
    CompressedData = 1,
}

/// gRPC 消息帧
#[derive(Clone, Debug)]
pub struct GrpcFrame {
    /// 是否压缩
    pub compressed: bool,
    /// 消息长度
    pub length: u32,
    /// 消息数据
    pub data: Vec<u8>,
}

impl GrpcFrame {
    /// 创建新帧
    pub fn new(data: Vec<u8>, compressed: bool) -> Self {
        Self {
            compressed,
            length: data.len() as u32,
            data,
        }
    }

    /// 编码帧
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5 + self.data.len());
        
        // 压缩标志 (1 byte)
        buf.push(if self.compressed { 1 } else { 0 });
        
        // 消息长度 (4 bytes, big-endian)
        buf.extend_from_slice(&self.length.to_be_bytes());
        
        // 消息数据
        buf.extend_from_slice(&self.data);
        
        buf
    }

    /// 解码帧
    pub fn decode(data: &[u8]) -> Result<(Self, usize), &'static str> {
        if data.len() < 5 {
            return Err("数据太短");
        }
        
        let compressed = data[0] != 0;
        let length = u32::from_be_bytes([data[1], data[2], data[3], data[4]]);
        
        let total_len = 5 + length as usize;
        if data.len() < total_len {
            return Err("数据不完整");
        }
        
        Ok((
            Self {
                compressed,
                length,
                data: data[5..total_len].to_vec(),
            },
            total_len,
        ))
    }
}

/// gRPC 请求构建器
pub struct GrpcRequestBuilder {
    config: GrpcDisguiseConfig,
}

impl GrpcRequestBuilder {
    pub fn new(config: GrpcDisguiseConfig) -> Self {
        Self { config }
    }

    /// 构建 gRPC 请求头
    pub fn build_headers(&self) -> Vec<(String, String)> {
        let mut headers = Vec::new();
        
        // 必需的 gRPC 头
        headers.push((":method".to_string(), "POST".to_string()));
        headers.push((
            ":path".to_string(),
            format!(
                "/{}.{}/{}",
                self.config.package_name,
                self.config.service_name,
                self.config.method_name
            ),
        ));
        headers.push((":scheme".to_string(), "https".to_string()));
        
        if self.config.grpc_web {
            headers.push(("content-type".to_string(), "application/grpc-web+proto".to_string()));
        } else {
            headers.push(("content-type".to_string(), "application/grpc".to_string()));
        }
        
        headers.push(("te".to_string(), "trailers".to_string()));
        headers.push(("grpc-encoding".to_string(), "identity".to_string()));
        headers.push(("grpc-accept-encoding".to_string(), "identity,deflate,gzip".to_string()));
        
        // 根据伪装模式添加特定头
        match self.config.disguise_mode {
            GrpcDisguiseMode::GoogleCloud => {
                headers.push(("x-goog-api-client".to_string(), "gl-go/1.21.0 grpc/1.59.0".to_string()));
                headers.push(("x-goog-request-params".to_string(), "project=my-project".to_string()));
            }
            GrpcDisguiseMode::AwsApi => {
                headers.push(("x-amz-target".to_string(), "AmazonEC2.DescribeInstances".to_string()));
                headers.push(("x-amz-date".to_string(), chrono_timestamp()));
            }
            GrpcDisguiseMode::AzureApi => {
                headers.push(("x-ms-client-request-id".to_string(), uuid_v4()));
                headers.push(("x-ms-version".to_string(), "2023-11-03".to_string()));
            }
            GrpcDisguiseMode::KubernetesApi => {
                headers.push(("x-kubernetes-pf-flow-schema-uid".to_string(), uuid_v4()));
            }
            GrpcDisguiseMode::Generic => {}
        }
        
        // 用户自定义元数据
        for (key, value) in &self.config.metadata {
            headers.push((format!("grpc-metadata-{}", key), value.clone()));
        }
        
        // User-Agent
        headers.push(("user-agent".to_string(), random_grpc_user_agent()));
        
        headers
    }

    /// 构建 HTTP/1.1 gRPC-Web 请求
    pub fn build_grpc_web_request(&self, data: &[u8]) -> Vec<u8> {
        let frame = GrpcFrame::new(data.to_vec(), false);
        let frame_data = frame.encode();
        
        let path = format!(
            "/{}.{}/{}",
            self.config.package_name,
            self.config.service_name,
            self.config.method_name
        );
        
        let request = format!(
            "POST {} HTTP/1.1\r\n\
             Content-Type: application/grpc-web+proto\r\n\
             Content-Length: {}\r\n\
             X-Grpc-Web: 1\r\n\
             X-User-Agent: grpc-web-javascript/0.1\r\n\
             User-Agent: {}\r\n\
             \r\n",
            path,
            frame_data.len(),
            random_grpc_user_agent()
        );
        
        let mut result = request.into_bytes();
        result.extend(frame_data);
        result
    }

    /// 封装数据为 gRPC 帧
    pub fn wrap_data(&self, data: &[u8]) -> Vec<u8> {
        let frame = GrpcFrame::new(data.to_vec(), false);
        frame.encode()
    }
}

/// gRPC 响应解析器
pub struct GrpcResponseParser {
    buffer: Vec<u8>,
    grpc_web: bool,
}

impl GrpcResponseParser {
    pub fn new(grpc_web: bool) -> Self {
        Self {
            buffer: Vec::new(),
            grpc_web,
        }
    }

    /// 解析响应
    pub fn parse(&mut self, data: &[u8]) -> Result<Vec<Vec<u8>>, &'static str> {
        self.buffer.extend_from_slice(data);
        
        let mut messages = Vec::new();
        
        while self.buffer.len() >= 5 {
            match GrpcFrame::decode(&self.buffer) {
                Ok((frame, consumed)) => {
                    messages.push(frame.data);
                    self.buffer = self.buffer[consumed..].to_vec();
                }
                Err(_) => break,
            }
        }
        
        Ok(messages)
    }

    /// 解析 gRPC 状态
    pub fn parse_status(trailers: &str) -> GrpcStatus {
        let mut status = GrpcStatus::default();
        
        for line in trailers.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_lowercase();
                let value = value.trim();
                
                match key.as_str() {
                    "grpc-status" => {
                        status.code = value.parse().unwrap_or(0);
                    }
                    "grpc-message" => {
                        status.message = value.to_string();
                    }
                    _ => {}
                }
            }
        }
        
        status
    }
}

/// gRPC 状态
#[derive(Clone, Debug, Default)]
pub struct GrpcStatus {
    pub code: i32,
    pub message: String,
}

impl GrpcStatus {
    pub fn ok() -> Self {
        Self {
            code: 0,
            message: String::new(),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.code == 0
    }
}

/// gRPC 状态码
pub mod status_code {
    pub const OK: i32 = 0;
    pub const CANCELLED: i32 = 1;
    pub const UNKNOWN: i32 = 2;
    pub const INVALID_ARGUMENT: i32 = 3;
    pub const DEADLINE_EXCEEDED: i32 = 4;
    pub const NOT_FOUND: i32 = 5;
    pub const ALREADY_EXISTS: i32 = 6;
    pub const PERMISSION_DENIED: i32 = 7;
    pub const RESOURCE_EXHAUSTED: i32 = 8;
    pub const FAILED_PRECONDITION: i32 = 9;
    pub const ABORTED: i32 = 10;
    pub const OUT_OF_RANGE: i32 = 11;
    pub const UNIMPLEMENTED: i32 = 12;
    pub const INTERNAL: i32 = 13;
    pub const UNAVAILABLE: i32 = 14;
    pub const DATA_LOSS: i32 = 15;
    pub const UNAUTHENTICATED: i32 = 16;
}

/// 生成随机 gRPC User-Agent
fn random_grpc_user_agent() -> String {
    let agents = [
        "grpc-go/1.59.0",
        "grpc-java/1.60.0",
        "grpc-node/1.9.0",
        "grpc-python/1.59.0",
        "grpc-c/35.0.0",
    ];
    
    let mut rng = rand::thread_rng();
    agents[rng.gen_range(0..agents.len())].to_string()
}

/// 生成 UUID v4
fn uuid_v4() -> String {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// 生成 AWS 风格时间戳
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // 简化的 ISO 8601 格式
    format!("{}T000000Z", now / 86400 * 86400)
}

/// Protobuf 简单编码器（用于伪装）
pub struct ProtobufEncoder;

impl ProtobufEncoder {
    /// 编码 varint
    pub fn encode_varint(mut value: u64) -> Vec<u8> {
        let mut buf = Vec::new();
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            buf.push(byte);
            if value == 0 {
                break;
            }
        }
        buf
    }

    /// 编码字符串字段
    pub fn encode_string(field_number: u32, value: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        
        // 字段标签 (field_number << 3 | wire_type)
        let tag = (field_number << 3) | 2; // wire_type 2 = length-delimited
        buf.extend(Self::encode_varint(tag as u64));
        
        // 长度
        buf.extend(Self::encode_varint(value.len() as u64));
        
        // 数据
        buf.extend_from_slice(value.as_bytes());
        
        buf
    }

    /// 编码字节字段
    pub fn encode_bytes(field_number: u32, value: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        
        let tag = (field_number << 3) | 2;
        buf.extend(Self::encode_varint(tag as u64));
        buf.extend(Self::encode_varint(value.len() as u64));
        buf.extend_from_slice(value);
        
        buf
    }

    /// 创建伪装的请求消息
    pub fn create_tunnel_request(data: &[u8]) -> Vec<u8> {
        let mut msg = Vec::new();
        
        // field 1: request_id (string)
        msg.extend(Self::encode_string(1, &uuid_v4()));
        
        // field 2: payload (bytes)
        msg.extend(Self::encode_bytes(2, data));
        
        // field 3: timestamp (varint)
        let tag = (3 << 3) | 0; // wire_type 0 = varint
        msg.extend(Self::encode_varint(tag as u64));
        msg.extend(Self::encode_varint(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        ));
        
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_frame() {
        let data = b"hello world";
        let frame = GrpcFrame::new(data.to_vec(), false);
        
        let encoded = frame.encode();
        assert_eq!(encoded[0], 0); // not compressed
        assert_eq!(u32::from_be_bytes([encoded[1], encoded[2], encoded[3], encoded[4]]), 11);
        
        let (decoded, consumed) = GrpcFrame::decode(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.data, data);
    }

    #[test]
    fn test_request_builder() {
        let config = GrpcDisguiseConfig::default();
        let builder = GrpcRequestBuilder::new(config);
        
        let headers = builder.build_headers();
        assert!(headers.iter().any(|(k, _)| k == "content-type"));
        assert!(headers.iter().any(|(k, _)| k == ":path"));
    }

    #[test]
    fn test_protobuf_encoder() {
        let data = b"test data";
        let msg = ProtobufEncoder::create_tunnel_request(data);
        
        assert!(!msg.is_empty());
        // 验证包含数据
        assert!(msg.windows(data.len()).any(|w| w == data));
    }

    #[test]
    fn test_varint_encoding() {
        assert_eq!(ProtobufEncoder::encode_varint(0), vec![0]);
        assert_eq!(ProtobufEncoder::encode_varint(1), vec![1]);
        assert_eq!(ProtobufEncoder::encode_varint(127), vec![127]);
        assert_eq!(ProtobufEncoder::encode_varint(128), vec![0x80, 0x01]);
    }
}
