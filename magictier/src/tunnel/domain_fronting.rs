//! 域前置 (Domain Fronting) 模块 - 利用 CDN 隐藏真实目标
//!
//! 原理：
//! 1. TLS SNI 使用合法域名（如 cdn.example.com）
//! 2. HTTP Host 头使用真实目标域名
//! 3. CDN 会将请求转发到 Host 指定的后端
//! 4. 深信服只能看到 SNI 中的合法域名
//!
//! 支持的 CDN：
//! - Cloudflare (部分支持)
//! - Fastly
//! - Azure CDN
//! - Akamai
//! - 阿里云 CDN
//! - 腾讯云 CDN

use std::collections::HashMap;

/// 域前置配置
#[derive(Clone, Debug)]
pub struct DomainFrontingConfig {
    /// 前置域名 (SNI 中使用)
    pub front_domain: String,
    /// 真实目标域名 (Host 头中使用)
    pub real_domain: String,
    /// CDN 类型
    pub cdn_type: CdnType,
    /// 是否启用
    pub enabled: bool,
    /// 自定义请求头
    pub custom_headers: HashMap<String, String>,
}

impl Default for DomainFrontingConfig {
    fn default() -> Self {
        Self {
            front_domain: String::new(),
            real_domain: String::new(),
            cdn_type: CdnType::Generic,
            enabled: false,
            custom_headers: HashMap::new(),
        }
    }
}

impl DomainFrontingConfig {
    /// 从 URL 参数解析配置
    pub fn from_url(url: &url::Url) -> Self {
        let mut config = Self::default();
        
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "front" | "front_domain" => {
                    config.front_domain = value.to_string();
                    config.enabled = true;
                }
                "real" | "real_domain" | "host" => {
                    config.real_domain = value.to_string();
                }
                "cdn" | "cdn_type" => {
                    config.cdn_type = CdnType::from_str(&value);
                }
                _ => {}
            }
        }
        
        config
    }
}

/// CDN 类型
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CdnType {
    /// 通用 CDN
    Generic,
    /// Cloudflare
    Cloudflare,
    /// Fastly
    Fastly,
    /// Azure CDN
    Azure,
    /// Akamai
    Akamai,
    /// 阿里云 CDN
    Aliyun,
    /// 腾讯云 CDN
    Tencent,
    /// AWS CloudFront
    CloudFront,
}

impl CdnType {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "cloudflare" | "cf" => Self::Cloudflare,
            "fastly" => Self::Fastly,
            "azure" => Self::Azure,
            "akamai" => Self::Akamai,
            "aliyun" | "alibaba" => Self::Aliyun,
            "tencent" | "qcloud" => Self::Tencent,
            "cloudfront" | "aws" => Self::CloudFront,
            _ => Self::Generic,
        }
    }
}

/// 域前置请求构建器
pub struct DomainFrontingRequest {
    config: DomainFrontingConfig,
}

impl DomainFrontingRequest {
    pub fn new(config: DomainFrontingConfig) -> Self {
        Self { config }
    }

    /// 获取 TLS SNI 域名
    pub fn get_sni(&self) -> &str {
        &self.config.front_domain
    }

    /// 获取 HTTP Host 头
    pub fn get_host(&self) -> &str {
        &self.config.real_domain
    }

    /// 构建 HTTP 请求头
    pub fn build_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        
        // 基本头
        headers.insert("Host".to_string(), self.config.real_domain.clone());
        headers.insert("Connection".to_string(), "keep-alive".to_string());
        
        // 根据 CDN 类型添加特定头
        match self.config.cdn_type {
            CdnType::Cloudflare => {
                headers.insert("CF-Connecting-IP".to_string(), "127.0.0.1".to_string());
            }
            CdnType::CloudFront => {
                headers.insert("X-Forwarded-For".to_string(), "127.0.0.1".to_string());
            }
            CdnType::Aliyun => {
                headers.insert("X-Real-IP".to_string(), "127.0.0.1".to_string());
            }
            _ => {}
        }
        
        // 添加自定义头
        for (k, v) in &self.config.custom_headers {
            headers.insert(k.clone(), v.clone());
        }
        
        headers
    }

    /// 构建 HTTP CONNECT 请求
    pub fn build_connect_request(&self, path: &str) -> Vec<u8> {
        let headers = self.build_headers();
        let mut request = format!(
            "CONNECT {} HTTP/1.1\r\nHost: {}\r\n",
            path,
            self.config.real_domain
        );
        
        for (k, v) in &headers {
            if k != "Host" {
                request.push_str(&format!("{}: {}\r\n", k, v));
            }
        }
        request.push_str("\r\n");
        
        request.into_bytes()
    }

    /// 构建 WebSocket 升级请求
    pub fn build_websocket_upgrade(&self, path: &str) -> Vec<u8> {
        use base64::Engine;
        
        let mut key = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
        let ws_key = base64::engine::general_purpose::STANDARD.encode(&key);
        
        let headers = self.build_headers();
        let mut request = format!(
            "GET {} HTTP/1.1\r\n\
             Host: {}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {}\r\n\
             Sec-WebSocket-Version: 13\r\n",
            path,
            self.config.real_domain,
            ws_key
        );
        
        for (k, v) in &headers {
            if k != "Host" && k != "Connection" {
                request.push_str(&format!("{}: {}\r\n", k, v));
            }
        }
        request.push_str("\r\n");
        
        request.into_bytes()
    }
}

/// 常用的前置域名列表（用于测试）
pub const COMMON_FRONT_DOMAINS: &[&str] = &[
    // Cloudflare
    "cdnjs.cloudflare.com",
    "ajax.cloudflare.com",
    // Azure
    "ajax.aspnetcdn.com",
    // Google (已不支持)
    // "www.google.com",
    // 阿里云
    "g.alicdn.com",
    // 腾讯云
    "imgcache.qq.com",
];

/// 检测域前置是否可用
pub async fn test_domain_fronting(
    front_domain: &str,
    real_domain: &str,
) -> Result<bool, std::io::Error> {
    use tokio::net::TcpStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    // 连接到前置域名
    let addr = format!("{}:443", front_domain);
    let mut stream = TcpStream::connect(&addr).await?;
    
    // 发送简单的 HTTP 请求
    let request = format!(
        "GET / HTTP/1.1\r\n\
         Host: {}\r\n\
         Connection: close\r\n\
         \r\n",
        real_domain
    );
    
    stream.write_all(request.as_bytes()).await?;
    
    let mut response = vec![0u8; 1024];
    let n = stream.read(&mut response).await?;
    
    // 检查是否收到有效响应
    let response_str = String::from_utf8_lossy(&response[..n]);
    Ok(response_str.contains("HTTP/1.1") || response_str.contains("HTTP/2"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_url() {
        let url = url::Url::parse(
            "https://example.com?front=cdn.example.com&real=target.com&cdn=cloudflare"
        ).unwrap();
        
        let config = DomainFrontingConfig::from_url(&url);
        assert_eq!(config.front_domain, "cdn.example.com");
        assert_eq!(config.real_domain, "target.com");
        assert_eq!(config.cdn_type, CdnType::Cloudflare);
        assert!(config.enabled);
    }

    #[test]
    fn test_build_headers() {
        let config = DomainFrontingConfig {
            front_domain: "cdn.example.com".to_string(),
            real_domain: "target.com".to_string(),
            cdn_type: CdnType::Cloudflare,
            enabled: true,
            custom_headers: HashMap::new(),
        };
        
        let req = DomainFrontingRequest::new(config);
        let headers = req.build_headers();
        
        assert_eq!(headers.get("Host").unwrap(), "target.com");
        assert!(headers.contains_key("CF-Connecting-IP"));
    }

    #[test]
    fn test_websocket_upgrade() {
        let config = DomainFrontingConfig {
            front_domain: "cdn.example.com".to_string(),
            real_domain: "target.com".to_string(),
            cdn_type: CdnType::Generic,
            enabled: true,
            custom_headers: HashMap::new(),
        };
        
        let req = DomainFrontingRequest::new(config);
        let upgrade = req.build_websocket_upgrade("/ws");
        let upgrade_str = String::from_utf8_lossy(&upgrade);
        
        assert!(upgrade_str.contains("Upgrade: websocket"));
        assert!(upgrade_str.contains("Host: target.com"));
    }
}
