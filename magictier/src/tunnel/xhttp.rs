//! XHTTP 协议模块 - Xray 2024-2025 最新协议 (Beyond REALITY)
//!
//! XHTTP 是 Xray 团队在 2024 年底发布的下一代传输协议，号称 "Beyond REALITY"：
//!
//! ## 核心特性
//! 1. **可过 CDN** - 支持 Cloudflare、AWS CloudFront 等 CDN
//! 2. **流量分离** - 上传和下载可走不同路径/IP
//! 3. **QUIC H3 支持** - 可使用 HTTP/3
//! 4. **XMUX 多路复用** - 降低延迟，提高效率
//! 5. **Header Padding** - 减少流量特征
//! 6. **gRPC 伪装** - stream-up 模式默认伪装成 gRPC
//!
//! ## 传输模式
//! - `packet-up`: 分包上传 + 流式下载（最佳兼容性）
//! - `stream-up`: 流式上传 + 流式下载（最佳效率，需要服务端支持）
//! - `stream-one`: 单连接双向流（HTTP/2 场景）
//!
//! ## 与 REALITY 的对比
//! | 特性 | REALITY | XHTTP |
//! |------|---------|-------|
//! | CDN 支持 | ❌ | ✅ |
//! | 流量分离 | ❌ | ✅ |
//! | QUIC H3 | ❌ | ✅ |
//! | 抗检测 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐+ |
//!
//! 参考: https://github.com/XTLS/Xray-core/discussions/4113

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use rand::Rng;
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

/// XHTTP 配置
#[derive(Clone, Debug)]
pub struct XHttpConfig {
    /// 是否启用
    pub enabled: bool,
    /// 请求路径
    pub path: String,
    /// Host 头
    pub host: String,
    /// 请求方法
    pub method: XHttpMethod,
    /// 自定义请求头
    pub headers: HashMap<String, String>,
    /// 是否启用分块传输
    pub chunked: bool,
    /// 是否启用长轮询
    pub long_polling: bool,
    /// 长轮询超时（秒）
    pub polling_timeout: u32,
    /// 是否伪装成特定网站
    pub disguise: Option<DisguiseTarget>,
    /// 传输模式
    pub mode: XHttpMode,
    /// XMUX 配置
    pub xmux: XMuxConfig,
    /// Header Padding 配置
    pub header_padding: HeaderPaddingConfig,
    /// 下行路径（流量分离时使用）
    pub download_path: Option<String>,
    /// 下行 Host（流量分离时使用）
    pub download_host: Option<String>,
    /// HTTP 版本
    pub http_version: HttpVersion,
    /// 额外配置 (extra scheme)
    pub extra: Option<ExtraConfig>,
}

impl Default for XHttpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/".to_string(),
            host: "www.example.com".to_string(),
            method: XHttpMethod::Post,
            headers: HashMap::new(),
            chunked: true,
            long_polling: true,
            polling_timeout: 30,
            disguise: Some(DisguiseTarget::GoogleApi),
            mode: XHttpMode::PacketUp,
            xmux: XMuxConfig::default(),
            header_padding: HeaderPaddingConfig::default(),
            download_path: None,
            download_host: None,
            http_version: HttpVersion::H2,
            extra: None,
        }
    }
}

/// XHTTP 传输模式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum XHttpMode {
    /// 分包上传 + 流式下载（最佳兼容性，可过大部分 CDN）
    PacketUp,
    /// 流式上传 + 流式下载（最佳效率，需要服务端支持 gRPC）
    StreamUp,
    /// 单连接双向流（HTTP/2 场景，替代传统 HTTP transport）
    StreamOne,
}

impl XHttpMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            XHttpMode::PacketUp => "packet-up",
            XHttpMode::StreamUp => "stream-up",
            XHttpMode::StreamOne => "stream-one",
        }
    }
}

/// HTTP 版本
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HttpVersion {
    /// HTTP/1.1
    H1,
    /// HTTP/2
    H2,
    /// HTTP/3 (QUIC)
    H3,
}

/// XMUX 多路复用配置
#[derive(Clone, Debug)]
pub struct XMuxConfig {
    /// 是否启用
    pub enabled: bool,
    /// 最大并发流数
    pub max_concurrency: u32,
    /// 最大连接数
    pub max_connections: u32,
    /// 连接复用时间（秒）
    pub connection_reuse_time: u32,
}

impl Default for XMuxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrency: 8,
            max_connections: 4,
            connection_reuse_time: 300,
        }
    }
}

/// Header Padding 配置（减少流量特征）
#[derive(Clone, Debug)]
pub struct HeaderPaddingConfig {
    /// 是否启用
    pub enabled: bool,
    /// 最小 padding 长度
    pub min_length: usize,
    /// 最大 padding 长度
    pub max_length: usize,
}

impl Default for HeaderPaddingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_length: 100,
            max_length: 1000,
        }
    }
}

/// Extra 配置（用于分享配置）
#[derive(Clone, Debug)]
pub struct ExtraConfig {
    /// 配置名称
    pub name: String,
    /// 配置版本
    pub version: u32,
    /// 额外参数
    pub params: HashMap<String, String>,
}

/// HTTP 方法
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum XHttpMethod {
    Get,
    Post,
    Put,
    Patch,
}

impl XHttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            XHttpMethod::Get => "GET",
            XHttpMethod::Post => "POST",
            XHttpMethod::Put => "PUT",
            XHttpMethod::Patch => "PATCH",
        }
    }
}

/// 伪装目标
#[derive(Clone, Debug, PartialEq)]
pub enum DisguiseTarget {
    /// Google API
    GoogleApi,
    /// Microsoft Graph API
    MicrosoftApi,
    /// Cloudflare API
    CloudflareApi,
    /// 通用 REST API
    GenericApi,
    /// 文件上传
    FileUpload,
    /// 自定义
    Custom(String),
}

/// XHTTP 请求构建器
pub struct XHttpRequestBuilder {
    config: XHttpConfig,
}

impl XHttpRequestBuilder {
    pub fn new(config: XHttpConfig) -> Self {
        Self { config }
    }

    /// 构建初始请求
    pub fn build_request(&self, payload: &[u8]) -> Vec<u8> {
        let mut request = String::new();
        
        // 请求行
        request.push_str(&format!(
            "{} {} HTTP/1.1\r\n",
            self.config.method.as_str(),
            self.config.path
        ));
        
        // Host
        request.push_str(&format!("Host: {}\r\n", self.config.host));
        
        // 根据伪装目标添加头
        self.add_disguise_headers(&mut request);
        
        // 自定义头
        for (key, value) in &self.config.headers {
            request.push_str(&format!("{}: {}\r\n", key, value));
        }
        
        // Content-Type 和 Content-Length
        if self.config.chunked {
            request.push_str("Transfer-Encoding: chunked\r\n");
        } else {
            request.push_str(&format!("Content-Length: {}\r\n", payload.len()));
        }
        
        // 连接保持
        request.push_str("Connection: keep-alive\r\n");
        
        // 结束头部
        request.push_str("\r\n");
        
        let mut result = request.into_bytes();
        
        // 添加 body
        if self.config.chunked {
            result.extend(self.encode_chunked(payload));
        } else {
            result.extend_from_slice(payload);
        }
        
        result
    }

    /// 添加伪装头
    fn add_disguise_headers(&self, request: &mut String) {
        match &self.config.disguise {
            Some(DisguiseTarget::GoogleApi) => {
                request.push_str("Content-Type: application/json\r\n");
                request.push_str("Accept: application/json\r\n");
                request.push_str("X-Goog-Api-Key: AIzaSyDummy_Key_For_Disguise\r\n");
                request.push_str("X-Goog-Api-Client: gl-js/1.0.0\r\n");
            }
            Some(DisguiseTarget::MicrosoftApi) => {
                request.push_str("Content-Type: application/json\r\n");
                request.push_str("Accept: application/json\r\n");
                request.push_str("Authorization: Bearer dummy_token\r\n");
                request.push_str("X-Ms-Client-Request-Id: ");
                request.push_str(&uuid_v4());
                request.push_str("\r\n");
            }
            Some(DisguiseTarget::CloudflareApi) => {
                request.push_str("Content-Type: application/json\r\n");
                request.push_str("Accept: application/json\r\n");
                request.push_str("X-Auth-Email: user@example.com\r\n");
                request.push_str("X-Auth-Key: dummy_api_key\r\n");
            }
            Some(DisguiseTarget::FileUpload) => {
                let boundary = generate_boundary();
                request.push_str(&format!(
                    "Content-Type: multipart/form-data; boundary={}\r\n",
                    boundary
                ));
            }
            Some(DisguiseTarget::GenericApi) | None => {
                request.push_str("Content-Type: application/octet-stream\r\n");
                request.push_str("Accept: */*\r\n");
            }
            Some(DisguiseTarget::Custom(ct)) => {
                request.push_str(&format!("Content-Type: {}\r\n", ct));
            }
        }
        
        // 通用头
        request.push_str(&format!("User-Agent: {}\r\n", random_user_agent()));
        request.push_str("Accept-Language: en-US,en;q=0.9\r\n");
        request.push_str("Accept-Encoding: gzip, deflate, br\r\n");
    }

    /// 编码为 chunked 格式
    fn encode_chunked(&self, data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        
        // 分成多个 chunk
        let mut rng = rand::thread_rng();
        let mut pos = 0;
        
        while pos < data.len() {
            let chunk_size = rng.gen_range(100..1000).min(data.len() - pos);
            let chunk = &data[pos..pos + chunk_size];
            
            // chunk 大小（十六进制）
            result.extend_from_slice(format!("{:x}\r\n", chunk_size).as_bytes());
            result.extend_from_slice(chunk);
            result.extend_from_slice(b"\r\n");
            
            pos += chunk_size;
        }
        
        // 结束 chunk
        result.extend_from_slice(b"0\r\n\r\n");
        
        result
    }

    /// 构建长轮询请求
    pub fn build_polling_request(&self) -> Vec<u8> {
        let mut request = String::new();
        
        request.push_str(&format!(
            "GET {} HTTP/1.1\r\n",
            self.config.path
        ));
        request.push_str(&format!("Host: {}\r\n", self.config.host));
        request.push_str(&format!("User-Agent: {}\r\n", random_user_agent()));
        request.push_str("Accept: text/event-stream\r\n");
        request.push_str("Cache-Control: no-cache\r\n");
        request.push_str("Connection: keep-alive\r\n");
        request.push_str("\r\n");
        
        request.into_bytes()
    }
}

/// XHTTP 响应解析器
pub struct XHttpResponseParser {
    buffer: Vec<u8>,
    headers_parsed: bool,
    content_length: Option<usize>,
    chunked: bool,
}

impl XHttpResponseParser {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            headers_parsed: false,
            content_length: None,
            chunked: false,
        }
    }

    /// 解析响应
    pub fn parse(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>, &'static str> {
        self.buffer.extend_from_slice(data);
        
        if !self.headers_parsed {
            if let Some(header_end) = self.find_header_end() {
                self.parse_headers(&self.buffer[..header_end].to_vec())?;
                self.buffer = self.buffer[header_end + 4..].to_vec();
                self.headers_parsed = true;
            } else {
                return Ok(None);
            }
        }
        
        if self.chunked {
            self.parse_chunked()
        } else if let Some(len) = self.content_length {
            if self.buffer.len() >= len {
                let body = self.buffer[..len].to_vec();
                self.buffer = self.buffer[len..].to_vec();
                Ok(Some(body))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn find_header_end(&self) -> Option<usize> {
        for i in 0..self.buffer.len().saturating_sub(3) {
            if &self.buffer[i..i + 4] == b"\r\n\r\n" {
                return Some(i);
            }
        }
        None
    }

    fn parse_headers(&mut self, headers: &[u8]) -> Result<(), &'static str> {
        let headers_str = String::from_utf8_lossy(headers);
        
        for line in headers_str.lines() {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                if let Some(len_str) = line.split(':').nth(1) {
                    if let Ok(len) = len_str.trim().parse() {
                        self.content_length = Some(len);
                    }
                }
            } else if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
                self.chunked = true;
            }
        }
        
        Ok(())
    }

    fn parse_chunked(&mut self) -> Result<Option<Vec<u8>>, &'static str> {
        let mut result = Vec::new();
        let mut pos = 0;
        
        loop {
            // 查找 chunk 大小行
            let size_end = self.buffer[pos..].iter()
                .position(|&b| b == b'\r')
                .map(|p| pos + p);
            
            let size_end = match size_end {
                Some(e) => e,
                None => return Ok(None),
            };
            
            let size_str = String::from_utf8_lossy(&self.buffer[pos..size_end]);
            let chunk_size = usize::from_str_radix(size_str.trim(), 16)
                .map_err(|_| "Invalid chunk size")?;
            
            if chunk_size == 0 {
                // 最后一个 chunk
                self.buffer = self.buffer[size_end + 4..].to_vec();
                return Ok(Some(result));
            }
            
            let chunk_start = size_end + 2;
            let chunk_end = chunk_start + chunk_size;
            
            if self.buffer.len() < chunk_end + 2 {
                return Ok(None);
            }
            
            result.extend_from_slice(&self.buffer[chunk_start..chunk_end]);
            pos = chunk_end + 2;
        }
    }
}

/// XHTTP 流
pub struct XHttpStream<S> {
    inner: S,
    config: XHttpConfig,
    request_builder: XHttpRequestBuilder,
    response_parser: XHttpResponseParser,
}

impl<S: AsyncRead + AsyncWrite + Unpin> XHttpStream<S> {
    pub fn new(stream: S, config: XHttpConfig) -> Self {
        let request_builder = XHttpRequestBuilder::new(config.clone());
        Self {
            inner: stream,
            config,
            request_builder,
            response_parser: XHttpResponseParser::new(),
        }
    }

    /// 发送数据
    pub async fn send(&mut self, data: &[u8]) -> std::io::Result<()> {
        let request = self.request_builder.build_request(data);
        self.inner.write_all(&request).await?;
        self.inner.flush().await
    }

    /// 接收数据
    pub async fn recv(&mut self) -> std::io::Result<Vec<u8>> {
        let mut buf = [0u8; 4096];
        
        loop {
            let n = self.inner.read(&mut buf).await?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Connection closed",
                ));
            }
            
            match self.response_parser.parse(&buf[..n]) {
                Ok(Some(data)) => return Ok(data),
                Ok(None) => continue,
                Err(e) => return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )),
            }
        }
    }

    /// 启动长轮询
    pub async fn start_polling(&mut self) -> std::io::Result<()> {
        if self.config.long_polling {
            let request = self.request_builder.build_polling_request();
            self.inner.write_all(&request).await?;
            self.inner.flush().await?;
        }
        Ok(())
    }
}

/// 生成随机 User-Agent
fn random_user_agent() -> &'static str {
    const USER_AGENTS: &[&str] = &[
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0",
    ];
    
    USER_AGENTS[rand::thread_rng().gen_range(0..USER_AGENTS.len())]
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

/// 生成 multipart boundary
fn generate_boundary() -> String {
    let mut rng = rand::thread_rng();
    let mut boundary = String::from("----WebKitFormBoundary");
    for _ in 0..16 {
        boundary.push(rng.gen_range(b'a'..=b'z') as char);
    }
    boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_builder() {
        let config = XHttpConfig::default();
        let builder = XHttpRequestBuilder::new(config);
        
        let request = builder.build_request(b"test data");
        let request_str = String::from_utf8_lossy(&request);
        
        assert!(request_str.contains("POST"));
        assert!(request_str.contains("Host:"));
        assert!(request_str.contains("Transfer-Encoding: chunked"));
    }

    #[test]
    fn test_chunked_encoding() {
        let config = XHttpConfig {
            chunked: true,
            ..Default::default()
        };
        let builder = XHttpRequestBuilder::new(config);
        
        let data = b"Hello, World!";
        let chunked = builder.encode_chunked(data);
        
        // 验证以 0\r\n\r\n 结尾
        assert!(chunked.ends_with(b"0\r\n\r\n"));
    }

    #[test]
    fn test_uuid_v4() {
        let uuid = uuid_v4();
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.chars().filter(|&c| c == '-').count(), 4);
    }
}

// ============================================
// XHTTP 高级功能 (2024-2025)
// ============================================

/// XMUX 会话管理器
pub struct XMuxManager {
    config: XMuxConfig,
    session_counter: AtomicU64,
    active_sessions: Arc<Mutex<HashMap<u64, XMuxSession>>>,
}

impl XMuxManager {
    pub fn new(config: XMuxConfig) -> Self {
        Self {
            config,
            session_counter: AtomicU64::new(0),
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 创建新会话
    pub async fn create_session(&self) -> u64 {
        let session_id = self.session_counter.fetch_add(1, Ordering::SeqCst);
        let session = XMuxSession {
            id: session_id,
            stream_count: 0,
            created_at: std::time::Instant::now(),
        };
        
        let mut sessions = self.active_sessions.lock().await;
        sessions.insert(session_id, session);
        session_id
    }

    /// 获取可用会话（复用或创建新的）
    pub async fn get_or_create_session(&self) -> u64 {
        let sessions = self.active_sessions.lock().await;
        
        // 查找可复用的会话
        for (id, session) in sessions.iter() {
            if session.stream_count < self.config.max_concurrency as usize {
                return *id;
            }
        }
        
        drop(sessions);
        self.create_session().await
    }
}

/// XMUX 会话
struct XMuxSession {
    id: u64,
    stream_count: usize,
    created_at: std::time::Instant,
}

/// 流量分离管理器
pub struct SplitTrafficManager {
    /// 上行配置
    upload_config: XHttpConfig,
    /// 下行配置
    download_config: XHttpConfig,
}

impl SplitTrafficManager {
    /// 创建流量分离管理器
    pub fn new(base_config: XHttpConfig) -> Self {
        let mut download_config = base_config.clone();
        
        // 如果配置了独立的下行路径
        if let Some(ref path) = base_config.download_path {
            download_config.path = path.clone();
        }
        if let Some(ref host) = base_config.download_host {
            download_config.host = host.clone();
        }
        
        Self {
            upload_config: base_config,
            download_config,
        }
    }

    /// 获取上行配置
    pub fn upload_config(&self) -> &XHttpConfig {
        &self.upload_config
    }

    /// 获取下行配置
    pub fn download_config(&self) -> &XHttpConfig {
        &self.download_config
    }
}

/// Header Padding 生成器
pub struct HeaderPaddingGenerator {
    config: HeaderPaddingConfig,
}

impl HeaderPaddingGenerator {
    pub fn new(config: HeaderPaddingConfig) -> Self {
        Self { config }
    }

    /// 生成随机 padding header
    pub fn generate(&self) -> (String, String) {
        if !self.config.enabled {
            return (String::new(), String::new());
        }

        let mut rng = rand::thread_rng();
        let length = rng.gen_range(self.config.min_length..=self.config.max_length);
        
        // 生成随机 header 名称（看起来像真实的 header）
        let header_names = [
            "X-Padding", "X-Request-Id", "X-Correlation-Id", 
            "X-Trace-Id", "X-Session-Token", "X-Client-Data",
        ];
        let header_name = header_names[rng.gen_range(0..header_names.len())];
        
        // 生成随机值（Base64 编码的随机数据）
        let random_bytes: Vec<u8> = (0..length).map(|_| rng.gen()).collect();
        let header_value = base64_encode(&random_bytes);
        
        (header_name.to_string(), header_value)
    }
}

/// gRPC 伪装头生成器（用于 stream-up 模式）
pub struct GrpcDisguiseHeaders;

impl GrpcDisguiseHeaders {
    /// 生成 gRPC 风格的请求头
    pub fn generate() -> HashMap<String, String> {
        let mut headers = HashMap::new();
        
        headers.insert("Content-Type".to_string(), "application/grpc".to_string());
        headers.insert("TE".to_string(), "trailers".to_string());
        headers.insert("Grpc-Accept-Encoding".to_string(), "identity,deflate,gzip".to_string());
        headers.insert("Grpc-Timeout".to_string(), "30S".to_string());
        
        headers
    }
}

/// CDN 配置预设
pub struct CdnPresets;

impl CdnPresets {
    /// Cloudflare 优化配置
    pub fn cloudflare() -> XHttpConfig {
        XHttpConfig {
            mode: XHttpMode::PacketUp,  // CF 对 packet-up 支持最好
            http_version: HttpVersion::H2,
            header_padding: HeaderPaddingConfig {
                enabled: true,
                min_length: 100,
                max_length: 500,
            },
            xmux: XMuxConfig {
                enabled: true,
                max_concurrency: 8,
                max_connections: 4,
                connection_reuse_time: 90,  // CF 100秒超时，留点余量
            },
            ..Default::default()
        }
    }

    /// AWS CloudFront 优化配置
    pub fn cloudfront() -> XHttpConfig {
        XHttpConfig {
            mode: XHttpMode::PacketUp,
            http_version: HttpVersion::H2,
            ..Default::default()
        }
    }

    /// 直连（无 CDN）优化配置
    pub fn direct() -> XHttpConfig {
        XHttpConfig {
            mode: XHttpMode::StreamUp,  // 直连可以用 stream-up，效率最高
            http_version: HttpVersion::H2,
            ..Default::default()
        }
    }

    /// REALITY + XHTTP 组合配置
    pub fn with_reality() -> XHttpConfig {
        XHttpConfig {
            mode: XHttpMode::StreamOne,
            http_version: HttpVersion::H2,
            disguise: Some(DisguiseTarget::MicrosoftApi),
            ..Default::default()
        }
    }
}

/// Base64 编码（简单实现）
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    let mut result = String::new();
    let mut i = 0;
    
    while i < data.len() {
        let b0 = data[i] as usize;
        let b1 = if i + 1 < data.len() { data[i + 1] as usize } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as usize } else { 0 };
        
        result.push(ALPHABET[(b0 >> 2) & 0x3f] as char);
        result.push(ALPHABET[((b0 << 4) | (b1 >> 4)) & 0x3f] as char);
        
        if i + 1 < data.len() {
            result.push(ALPHABET[((b1 << 2) | (b2 >> 6)) & 0x3f] as char);
        } else {
            result.push('=');
        }
        
        if i + 2 < data.len() {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
        
        i += 3;
    }
    
    result
}

/// XHTTP URL 解析器
pub struct XHttpUrlParser;

impl XHttpUrlParser {
    /// 从 URL 解析 XHTTP 配置
    /// 格式: xhttp://host:port/path?mode=packet-up&download=...
    pub fn parse(url: &str) -> Result<XHttpConfig, &'static str> {
        let url = url::Url::parse(url).map_err(|_| "Invalid URL")?;
        
        let mut config = XHttpConfig::default();
        
        // 解析 host
        if let Some(host) = url.host_str() {
            config.host = host.to_string();
        }
        
        // 解析 path
        config.path = url.path().to_string();
        if config.path.is_empty() {
            config.path = "/".to_string();
        }
        
        // 解析查询参数
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "mode" => {
                    config.mode = match value.as_ref() {
                        "packet-up" => XHttpMode::PacketUp,
                        "stream-up" => XHttpMode::StreamUp,
                        "stream-one" => XHttpMode::StreamOne,
                        _ => XHttpMode::PacketUp,
                    };
                }
                "download" | "downloadPath" => {
                    config.download_path = Some(value.to_string());
                }
                "downloadHost" => {
                    config.download_host = Some(value.to_string());
                }
                "h3" | "quic" => {
                    if value == "true" || value == "1" {
                        config.http_version = HttpVersion::H3;
                    }
                }
                "xmux" => {
                    config.xmux.enabled = value == "true" || value == "1";
                }
                "maxConcurrency" => {
                    if let Ok(v) = value.parse() {
                        config.xmux.max_concurrency = v;
                    }
                }
                _ => {}
            }
        }
        
        Ok(config)
    }
}

#[cfg(test)]
mod advanced_tests {
    use super::*;

    #[test]
    fn test_xhttp_modes() {
        assert_eq!(XHttpMode::PacketUp.as_str(), "packet-up");
        assert_eq!(XHttpMode::StreamUp.as_str(), "stream-up");
        assert_eq!(XHttpMode::StreamOne.as_str(), "stream-one");
    }

    #[test]
    fn test_header_padding() {
        let config = HeaderPaddingConfig {
            enabled: true,
            min_length: 10,
            max_length: 20,
        };
        let generator = HeaderPaddingGenerator::new(config);
        let (name, value) = generator.generate();
        
        assert!(!name.is_empty());
        assert!(!value.is_empty());
    }

    #[test]
    fn test_cdn_presets() {
        let cf_config = CdnPresets::cloudflare();
        assert_eq!(cf_config.mode, XHttpMode::PacketUp);
        assert!(cf_config.xmux.connection_reuse_time < 100);
        
        let direct_config = CdnPresets::direct();
        assert_eq!(direct_config.mode, XHttpMode::StreamUp);
    }

    #[test]
    fn test_url_parser() {
        let config = XHttpUrlParser::parse("xhttp://example.com/api?mode=stream-up&h3=true").unwrap();
        assert_eq!(config.host, "example.com");
        assert_eq!(config.path, "/api");
        assert_eq!(config.mode, XHttpMode::StreamUp);
        assert_eq!(config.http_version, HttpVersion::H3);
    }

    #[test]
    fn test_grpc_disguise() {
        let headers = GrpcDisguiseHeaders::generate();
        assert_eq!(headers.get("Content-Type"), Some(&"application/grpc".to_string()));
        assert!(headers.contains_key("TE"));
    }

    #[tokio::test]
    async fn test_xmux_manager() {
        let config = XMuxConfig::default();
        let manager = XMuxManager::new(config);
        
        let session1 = manager.create_session().await;
        let session2 = manager.create_session().await;
        
        assert_ne!(session1, session2);
    }
}
