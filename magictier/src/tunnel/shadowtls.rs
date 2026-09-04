//! ShadowTLS v3 协议实现 - 完整版
//!
//! ShadowTLS 通过与真实 TLS 服务器完成握手来伪装流量：
//! 1. 客户端连接代理服务器
//! 2. 代理服务器与真实 TLS 服务器（如 cloudflare.com）完成握手
//! 3. 握手完成后，使用 HMAC 验证切换到代理模式
//! 4. 深信服看到的是完整的、真实的 TLS 握手

use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::{
    common::{setup_sokcet2, FramedReader, FramedWriter, TunnelWrapper},
    IpVersion, Tunnel, TunnelConnector, TunnelError, TunnelListener,
};

type HmacSha256 = Hmac<Sha256>;

/// ShadowTLS 配置
#[derive(Clone, Debug)]
pub struct ShadowTlsConfig {
    /// 密码 (用于 HMAC 验证)
    pub password: String,
    /// 伪装的 TLS 服务器地址
    pub handshake_server: String,
    /// 伪装的 TLS 服务器端口
    pub handshake_port: u16,
    /// SNI (Server Name Indication)
    pub sni: String,
}

impl Default for ShadowTlsConfig {
    fn default() -> Self {
        Self {
            password: "shadowtls_password".to_string(),
            handshake_server: "www.cloudflare.com".to_string(),
            handshake_port: 443,
            sni: "www.cloudflare.com".to_string(),
        }
    }
}

impl ShadowTlsConfig {
    /// 从 URL 参数解析配置
    /// 支持的参数:
    /// - password/pwd: 认证密码
    /// - handshake_server/hs: 伪装 TLS 服务器地址
    /// - handshake_port/hp: 伪装 TLS 服务器端口
    /// - sni: Server Name Indication
    pub fn from_url(url: &url::Url) -> Self {
        let mut config = Self::default();
        
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "password" | "pwd" => {
                    config.password = value.to_string();
                }
                "handshake_server" | "hs" => {
                    config.handshake_server = value.to_string();
                    // 如果没有单独设置 SNI，使用握手服务器作为 SNI
                    if config.sni == "www.cloudflare.com" {
                        config.sni = value.to_string();
                    }
                }
                "handshake_port" | "hp" => {
                    if let Ok(port) = value.parse() {
                        config.handshake_port = port;
                    }
                }
                "sni" => {
                    config.sni = value.to_string();
                }
                _ => {}
            }
        }
        
        config
    }
}

/// 计算 HMAC-SHA256
fn compute_hmac(password: &str, data: &[u8]) -> [u8; 8] {
    let mut mac = HmacSha256::new_from_slice(password.as_bytes())
        .expect("HMAC 可以接受任意长度的密钥");
    mac.update(data);
    
    let result = mac.finalize();
    let mut hmac = [0u8; 8];
    hmac.copy_from_slice(&result.into_bytes()[..8]);
    hmac
}

/// 验证 HMAC
fn verify_hmac(password: &str, data: &[u8], expected: &[u8]) -> bool {
    let computed = compute_hmac(password, data);
    computed[..] == expected[..8.min(expected.len())]
}

/// ShadowTLS v3 协议常量
const SHADOWTLS_HMAC_LEN: usize = 8;
#[allow(dead_code)]
const SHADOWTLS_CMD_DATA: u8 = 0x00;
const SHADOWTLS_CMD_SWITCH: u8 = 0x01;

/// 构建 TLS Client Hello
fn build_tls_client_hello(sni: &str, hmac_data: Option<&[u8]>) -> Vec<u8> {
    let mut hello = Vec::with_capacity(512);
    
    // TLS Record Header
    hello.push(0x16); // Handshake
    hello.extend_from_slice(&[0x03, 0x01]); // TLS 1.0
    
    let record_len_pos = hello.len();
    hello.extend_from_slice(&[0x00, 0x00]);
    
    // Handshake Header
    hello.push(0x01); // Client Hello
    let hs_len_pos = hello.len();
    hello.extend_from_slice(&[0x00, 0x00, 0x00]);
    
    // Client Version
    hello.extend_from_slice(&[0x03, 0x03]); // TLS 1.2
    
    // Random (32 bytes)
    let mut random = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut random);
    hello.extend_from_slice(&random);
    
    // Session ID (32 bytes) - 嵌入 HMAC
    hello.push(32);
    let mut session_id = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut session_id);
    if let Some(hmac) = hmac_data {
        session_id[0..8.min(hmac.len())].copy_from_slice(&hmac[..8.min(hmac.len())]);
    }
    hello.extend_from_slice(&session_id);
    
    // Cipher Suites
    hello.extend_from_slice(&[
        0x00, 0x08,
        0x13, 0x01, // TLS_AES_128_GCM_SHA256
        0x13, 0x02, // TLS_AES_256_GCM_SHA384
        0x13, 0x03, // TLS_CHACHA20_POLY1305_SHA256
        0xc0, 0x2f, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
    ]);
    
    // Compression Methods
    hello.extend_from_slice(&[0x01, 0x00]);
    
    // Extensions
    let mut extensions = Vec::new();
    
    // SNI
    let sni_bytes = sni.as_bytes();
    extensions.extend_from_slice(&[0x00, 0x00]);
    let sni_len = sni_bytes.len() + 5;
    extensions.extend_from_slice(&(sni_len as u16).to_be_bytes());
    extensions.extend_from_slice(&((sni_len - 2) as u16).to_be_bytes());
    extensions.push(0x00);
    extensions.extend_from_slice(&(sni_bytes.len() as u16).to_be_bytes());
    extensions.extend_from_slice(sni_bytes);
    
    // Supported Versions
    extensions.extend_from_slice(&[0x00, 0x2b, 0x00, 0x03, 0x02, 0x03, 0x04]);
    
    // Supported Groups
    extensions.extend_from_slice(&[
        0x00, 0x0a, 0x00, 0x06, 0x00, 0x04,
        0x00, 0x1d, 0x00, 0x17,
    ]);
    
    // Signature Algorithms
    extensions.extend_from_slice(&[
        0x00, 0x0d, 0x00, 0x08, 0x00, 0x06,
        0x04, 0x03, 0x05, 0x03, 0x06, 0x03,
    ]);
    
    // Key Share (X25519)
    let mut key_share = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key_share);
    extensions.extend_from_slice(&[0x00, 0x33, 0x00, 0x26, 0x00, 0x24]);
    extensions.extend_from_slice(&[0x00, 0x1d, 0x00, 0x20]);
    extensions.extend_from_slice(&key_share);
    
    hello.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    hello.extend_from_slice(&extensions);
    
    // 更新长度
    let record_len = hello.len() - 5;
    hello[record_len_pos..record_len_pos + 2].copy_from_slice(&(record_len as u16).to_be_bytes());
    
    let hs_len = hello.len() - hs_len_pos - 3;
    hello[hs_len_pos] = ((hs_len >> 16) & 0xff) as u8;
    hello[hs_len_pos + 1] = ((hs_len >> 8) & 0xff) as u8;
    hello[hs_len_pos + 2] = (hs_len & 0xff) as u8;
    
    hello
}

/// ShadowTLS 隧道连接器
pub struct ShadowTlsTunnelConnector {
    addr: url::Url,
    config: ShadowTlsConfig,
    ip_version: IpVersion,
    #[allow(dead_code)]
    bind_addrs: Vec<SocketAddr>,
}

impl ShadowTlsTunnelConnector {
    pub fn new(addr: url::Url, config: ShadowTlsConfig) -> Self {
        Self {
            addr,
            config,
            ip_version: IpVersion::Both,
            bind_addrs: Vec::new(),
        }
    }
    
    /// 执行 ShadowTLS 握手
    async fn do_handshake(&self, proxy_stream: &mut TcpStream) -> Result<(), TunnelError> {
        // 1. 连接真实的 TLS 服务器
        let handshake_addr = format!("{}:{}", self.config.handshake_server, self.config.handshake_port);
        let mut real_server = TcpStream::connect(&handshake_addr).await
            .map_err(|e| TunnelError::InternalError(format!("连接握手服务器失败: {:?}", e)))?;
        
        // 2. 构建带 HMAC 的 Client Hello
        let hmac = compute_hmac(&self.config.password, b"client_hello");
        let client_hello = build_tls_client_hello(&self.config.sni, Some(&hmac));
        
        // 3. 发送到真实服务器
        real_server.write_all(&client_hello).await?;
        
        // 4. 同时发送到代理服务器
        proxy_stream.write_all(&client_hello).await?;
        
        // 5. 转发真实服务器的响应
        let mut buf = [0u8; 8192];
        let mut handshake_done = false;
        
        while !handshake_done {
            let n = real_server.read(&mut buf).await?;
            if n == 0 {
                return Err(TunnelError::InternalError("握手服务器关闭连接".into()));
            }
            
            // 转发到代理服务器
            proxy_stream.write_all(&buf[..n]).await?;
            
            // 检查是否握手完成 (收到 Application Data 或 Change Cipher Spec)
            if buf[0] == 0x17 || buf[0] == 0x14 {
                handshake_done = true;
            }
        }
        
        // 6. 发送切换命令
        let switch_hmac = compute_hmac(&self.config.password, b"switch");
        let mut switch_cmd = Vec::new();
        switch_cmd.push(0x17); // Application Data
        switch_cmd.extend_from_slice(&[0x03, 0x03]);
        switch_cmd.extend_from_slice(&((SHADOWTLS_HMAC_LEN + 1) as u16).to_be_bytes());
        switch_cmd.extend_from_slice(&switch_hmac);
        switch_cmd.push(SHADOWTLS_CMD_SWITCH);
        
        proxy_stream.write_all(&switch_cmd).await?;
        
        // 关闭与真实服务器的连接
        drop(real_server);
        
        tracing::info!("ShadowTLS 握手完成");
        Ok(())
    }
}

#[async_trait::async_trait]
impl TunnelConnector for ShadowTlsTunnelConnector {
    async fn connect(&mut self) -> Result<Box<dyn Tunnel>, TunnelError> {
        let addr = super::check_scheme_and_get_socket_addr::<SocketAddr>(
            &self.addr,
            "shadowtls",
            self.ip_version,
        ).await?;

        let mut stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;

        // 执行 ShadowTLS 握手
        self.do_handshake(&mut stream).await?;

        let info = super::TunnelInfo {
            tunnel_type: "shadowtls".to_owned(),
            local_addr: Some(
                super::build_url_from_socket_addr(&stream.local_addr()?.to_string(), "shadowtls").into(),
            ),
            remote_addr: Some(self.addr.clone().into()),
        };

        let (r, w) = stream.into_split();
        Ok(Box::new(TunnelWrapper::new(
            FramedReader::new(r, 4096),
            FramedWriter::new(w),
            Some(info),
        )))
    }

    fn remote_url(&self) -> url::Url {
        self.addr.clone()
    }

    fn set_ip_version(&mut self, ip_version: IpVersion) {
        self.ip_version = ip_version;
    }

    fn set_bind_addrs(&mut self, addrs: Vec<SocketAddr>) {
        self.bind_addrs = addrs;
    }
}

/// ShadowTLS 隧道监听器
pub struct ShadowTlsTunnelListener {
    addr: url::Url,
    config: ShadowTlsConfig,
    listener: Option<TcpListener>,
}

impl ShadowTlsTunnelListener {
    pub fn new(addr: url::Url, config: ShadowTlsConfig) -> Self {
        Self {
            addr,
            config,
            listener: None,
        }
    }
    
    /// 验证客户端握手
    async fn verify_and_relay(
        &self,
        client: &mut TcpStream,
    ) -> Result<bool, TunnelError> {
        // 读取 Client Hello
        let mut buf = [0u8; 4096];
        let n = client.read(&mut buf).await?;
        
        if n < 50 || buf[0] != 0x16 {
            return Ok(false);
        }
        
        // 提取 Session ID 中的 HMAC (偏移 43)
        if n < 43 + SHADOWTLS_HMAC_LEN {
            return Ok(false);
        }
        
        let client_hmac = &buf[43..43 + SHADOWTLS_HMAC_LEN];
        
        if !verify_hmac(&self.config.password, b"client_hello", client_hmac) {
            return Ok(false);
        }
        
        // 连接真实服务器并转发握手
        let handshake_addr = format!("{}:{}", self.config.handshake_server, self.config.handshake_port);
        let mut real_server = TcpStream::connect(&handshake_addr).await
            .map_err(|e| TunnelError::InternalError(format!("连接握手服务器失败: {:?}", e)))?;
        
        // 清除 HMAC 后转发到真实服务器
        let mut clean_hello = buf[..n].to_vec();
        clean_hello[43..43 + SHADOWTLS_HMAC_LEN].fill(0);
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut clean_hello[43..43 + SHADOWTLS_HMAC_LEN]);
        real_server.write_all(&clean_hello).await?;
        
        // 转发真实服务器的响应
        let mut response_buf = [0u8; 8192];
        loop {
            let n = real_server.read(&mut response_buf).await?;
            if n == 0 {
                break;
            }
            
            client.write_all(&response_buf[..n]).await?;
            
            // 检查是否握手完成
            if response_buf[0] == 0x17 || response_buf[0] == 0x14 {
                break;
            }
        }
        
        // 等待切换命令
        let mut switch_buf = [0u8; 64];
        let n = client.read(&mut switch_buf).await?;
        
        if n < 5 + SHADOWTLS_HMAC_LEN + 1 {
            return Ok(false);
        }
        
        // 验证切换命令
        if switch_buf[0] != 0x17 {
            return Ok(false);
        }
        
        let switch_hmac = &switch_buf[5..5 + SHADOWTLS_HMAC_LEN];
        if !verify_hmac(&self.config.password, b"switch", switch_hmac) {
            return Ok(false);
        }
        
        if switch_buf[5 + SHADOWTLS_HMAC_LEN] != SHADOWTLS_CMD_SWITCH {
            return Ok(false);
        }
        
        drop(real_server);
        Ok(true)
    }
}

#[async_trait::async_trait]
impl TunnelListener for ShadowTlsTunnelListener {
    async fn listen(&mut self) -> Result<(), TunnelError> {
        let addr = super::check_scheme_and_get_socket_addr::<SocketAddr>(
            &self.addr,
            "shadowtls",
            IpVersion::Both,
        ).await?;

        let socket2_socket = socket2::Socket::new(
            socket2::Domain::for_address(addr),
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )?;
        setup_sokcet2(&socket2_socket, &addr)?;
        let socket = TcpSocket::from_std_stream(socket2_socket.into());

        self.addr.set_port(Some(socket.local_addr()?.port())).unwrap();
        self.listener = Some(socket.listen(1024)?);
        
        tracing::info!(
            "ShadowTLS 监听器启动: {}\n  握手服务器: {}:{}\n  密码: {}",
            self.addr,
            self.config.handshake_server,
            self.config.handshake_port,
            &self.config.password[..4.min(self.config.password.len())]
        );
        
        Ok(())
    }

    async fn accept(&mut self) -> Result<Box<dyn Tunnel>, TunnelError> {
        let listener = self.listener.as_ref().unwrap();
        
        loop {
            let (mut stream, peer_addr) = listener.accept().await?;
            stream.set_nodelay(true)?;
            
            match self.verify_and_relay(&mut stream).await {
                Ok(true) => {
                    let info = super::TunnelInfo {
                        tunnel_type: "shadowtls".to_owned(),
                        local_addr: Some(self.local_url().into()),
                        remote_addr: Some(
                            super::build_url_from_socket_addr(&peer_addr.to_string(), "shadowtls").into(),
                        ),
                    };
                    
                    let (r, w) = stream.into_split();
                    return Ok(Box::new(TunnelWrapper::new(
                        FramedReader::new(r, 4096),
                        FramedWriter::new(w),
                        Some(info),
                    )));
                }
                Ok(false) | Err(_) => {
                    // 转发到真实服务器
                    let config = self.config.clone();
                    tokio::spawn(async move {
                        let addr = format!("{}:{}", config.handshake_server, config.handshake_port);
                        if let Ok(mut server) = TcpStream::connect(&addr).await {
                            let _ = tokio::io::copy_bidirectional(&mut stream, &mut server).await;
                        }
                    });
                }
            }
        }
    }

    fn local_url(&self) -> url::Url {
        self.addr.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac() {
        let password = "test_password";
        let data = b"hello world";
        
        let hmac1 = compute_hmac(password, data);
        let hmac2 = compute_hmac(password, data);
        
        assert_eq!(hmac1, hmac2);
        assert!(verify_hmac(password, data, &hmac1));
        
        let wrong_hmac = [0u8; 8];
        assert!(!verify_hmac(password, data, &wrong_hmac));
    }

    #[test]
    fn test_client_hello_build() {
        let hello = build_tls_client_hello("www.cloudflare.com", None);
        
        assert_eq!(hello[0], 0x16); // Handshake
        assert_eq!(hello[5], 0x01); // Client Hello
    }
}
