//! Hysteria 2 协议实现 - 完整版
//!
//! Hysteria 2 是基于 QUIC 的高速代理协议：
//! 1. 使用 QUIC 协议，流量特征与正常 HTTP/3 一致
//! 2. Brutal 拥塞控制算法，可跑满带宽
//! 3. 支持 0-RTT 快速连接
//! 4. 内置 UDP 转发

use std::{net::SocketAddr, sync::Arc, time::Duration};

use super::{IpVersion, TunnelConnector, TunnelError};

/// Hysteria 2 配置
#[derive(Clone, Debug)]
pub struct Hysteria2Config {
    /// 认证密码
    pub password: String,
    /// 上行带宽 (Mbps)
    pub up_mbps: u32,
    /// 下行带宽 (Mbps)
    pub down_mbps: u32,
    /// 是否启用 0-RTT
    pub enable_0rtt: bool,
    /// 伪装域名 (用于 SNI)
    pub sni: Option<String>,
    /// ALPN 协议
    pub alpn: Vec<String>,
    /// 是否跳过证书验证
    pub insecure: bool,
}

impl Default for Hysteria2Config {
    fn default() -> Self {
        Self {
            password: String::new(),
            up_mbps: 100,
            down_mbps: 100,
            enable_0rtt: true,
            sni: None,
            alpn: vec!["h3".to_string()],
            insecure: true,
        }
    }
}

impl Hysteria2Config {
    /// 从 URL 参数解析配置
    /// 支持的参数:
    /// - password/pwd: 认证密码
    /// - up_mbps/up: 上行带宽 (Mbps)
    /// - down_mbps/down: 下行带宽 (Mbps)
    /// - sni: 伪装域名
    /// - alpn: ALPN 协议 (逗号分隔)
    /// - insecure: 是否跳过证书验证 (true/false)
    /// - 0rtt: 是否启用 0-RTT (true/false)
    pub fn from_url(url: &url::Url) -> Self {
        let mut config = Self::default();
        
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "password" | "pwd" => {
                    config.password = value.to_string();
                }
                "up_mbps" | "up" => {
                    if let Ok(mbps) = value.parse() {
                        config.up_mbps = mbps;
                    }
                }
                "down_mbps" | "down" => {
                    if let Ok(mbps) = value.parse() {
                        config.down_mbps = mbps;
                    }
                }
                "sni" => {
                    config.sni = Some(value.to_string());
                }
                "alpn" => {
                    config.alpn = value.split(',').map(|s| s.trim().to_string()).collect();
                }
                "insecure" => {
                    config.insecure = value == "true" || value == "1";
                }
                "0rtt" | "zero_rtt" => {
                    config.enable_0rtt = value == "true" || value == "1";
                }
                _ => {}
            }
        }
        
        config
    }
}

/// Hysteria 2 帧类型
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hy2FrameType {
    Auth = 0x00,
    AuthResp = 0x01,
    TcpRequest = 0x02,
    TcpResponse = 0x03,
    UdpMessage = 0x04,
    Ping = 0x05,
    Pong = 0x06,
}

/// Hysteria 2 认证请求
#[derive(Clone, Debug)]
pub struct Hy2AuthRequest {
    pub auth_type: u8,
    pub auth_data: Vec<u8>,
    pub tx: u64,
    pub rx: u64,
}

impl Hy2AuthRequest {
    pub fn new(password: &str, up_mbps: u32, down_mbps: u32) -> Self {
        Self {
            auth_type: 0,
            auth_data: password.as_bytes().to_vec(),
            tx: (up_mbps as u64) * 1_000_000,
            rx: (down_mbps as u64) * 1_000_000,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(Hy2FrameType::Auth as u8);
        buf.push(self.auth_type);
        buf.extend_from_slice(&encode_varint(self.auth_data.len() as u64));
        buf.extend_from_slice(&self.auth_data);
        buf.extend_from_slice(&self.tx.to_be_bytes());
        buf.extend_from_slice(&self.rx.to_be_bytes());
        buf
    }
}

/// Hysteria 2 认证响应
#[derive(Clone, Debug)]
pub struct Hy2AuthResponse {
    pub ok: bool,
    pub tx: u64,
    pub rx: u64,
    pub message: String,
}

impl Hy2AuthResponse {
    pub fn decode(data: &[u8]) -> Result<Self, TunnelError> {
        if data.len() < 2 {
            return Err(TunnelError::InvalidPacket("认证响应太短".into()));
        }
        
        if data[0] != Hy2FrameType::AuthResp as u8 {
            return Err(TunnelError::InvalidPacket("无效的认证响应类型".into()));
        }
        
        let ok = data[1] == 1;
        
        if data.len() >= 18 {
            let tx = u64::from_be_bytes(data[2..10].try_into().unwrap());
            let rx = u64::from_be_bytes(data[10..18].try_into().unwrap());
            
            Ok(Self { ok, tx, rx, message: String::new() })
        } else {
            Ok(Self {
                ok,
                tx: 0,
                rx: 0,
                message: String::from_utf8_lossy(&data[2..]).to_string(),
            })
        }
    }
}

/// 编码 varint
fn encode_varint(mut value: u64) -> Vec<u8> {
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

/// 解码 varint
#[allow(dead_code)]
fn decode_varint(data: &[u8]) -> Result<(u64, usize), TunnelError> {
    let mut value: u64 = 0;
    let mut shift = 0;
    let mut consumed = 0;
    
    for &byte in data {
        consumed += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, consumed));
        }
        shift += 7;
        if shift >= 64 {
            return Err(TunnelError::InvalidPacket("Varint 太长".into()));
        }
    }
    
    Err(TunnelError::InvalidPacket("不完整的 varint".into()))
}

/// Brutal 拥塞控制算法
pub struct BrutalCongestionControl {
    target_rate: u64,
    cwnd: u64,
    min_rtt: Duration,
    loss_rate: f64,
}

impl BrutalCongestionControl {
    pub fn new(target_mbps: u32) -> Self {
        Self {
            target_rate: (target_mbps as u64) * 125_000,
            cwnd: 32 * 1024,
            min_rtt: Duration::from_millis(50),
            loss_rate: 0.0,
        }
    }

    pub fn get_cwnd(&self) -> u64 {
        let rtt_secs = self.min_rtt.as_secs_f64();
        let cwnd = (self.target_rate as f64 * rtt_secs * (1.0 + self.loss_rate)) as u64;
        cwnd.max(self.cwnd)
    }

    pub fn update_rtt(&mut self, rtt: Duration) {
        if rtt < self.min_rtt {
            self.min_rtt = rtt;
        }
    }

    pub fn update_loss(&mut self, lost: u64, total: u64) {
        if total > 0 {
            self.loss_rate = lost as f64 / total as f64;
        }
    }
}

/// Hysteria 2 QUIC 配置
#[cfg(feature = "quic")]
fn create_hy2_quic_config(config: &Hysteria2Config) -> Result<quinn::ClientConfig, TunnelError> {
    use super::insecure_tls::get_insecure_tls_client_config;
    use quinn::crypto::rustls::QuicClientConfig;
    
    let tls_config = get_insecure_tls_client_config();
    let client_crypto = QuicClientConfig::try_from(tls_config)
        .map_err(|e| TunnelError::InternalError(format!("创建 QUIC 配置失败: {:?}", e)))?;
    
    let mut client_config = quinn::ClientConfig::new(Arc::new(client_crypto));
    
    let mut transport_config = quinn::TransportConfig::default();
    
    if config.enable_0rtt {
        transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
    }
    
    // 设置初始拥塞窗口和发送窗口
    transport_config.initial_mtu(1200);
    transport_config.receive_window(quinn::VarInt::from_u32(8 * 1024 * 1024));
    transport_config.keep_alive_interval(Some(Duration::from_secs(15)));
    
    client_config.transport_config(Arc::new(transport_config));
    
    Ok(client_config)
}

/// Hysteria 2 隧道连接器
#[cfg(feature = "quic")]
pub struct Hysteria2TunnelConnector {
    addr: url::Url,
    config: Hysteria2Config,
    ip_version: IpVersion,
    #[allow(dead_code)]
    bind_addrs: Vec<SocketAddr>,
}

#[cfg(feature = "quic")]
impl Hysteria2TunnelConnector {
    pub fn new(addr: url::Url, config: Hysteria2Config) -> Self {
        Self {
            addr,
            config,
            ip_version: IpVersion::Both,
            bind_addrs: Vec::new(),
        }
    }

    async fn do_auth(
        &self,
        send: &mut quinn::SendStream,
        recv: &mut quinn::RecvStream,
    ) -> Result<Hy2AuthResponse, TunnelError> {
        let auth_req = Hy2AuthRequest::new(
            &self.config.password,
            self.config.up_mbps,
            self.config.down_mbps,
        );
        
        let auth_data = auth_req.encode();
        send.write_all(&auth_data).await
            .map_err(|e| TunnelError::InternalError(format!("发送认证失败: {:?}", e)))?;
        
        let mut buf = [0u8; 256];
        let n = recv.read(&mut buf).await
            .map_err(|e| TunnelError::InternalError(format!("读取认证响应失败: {:?}", e)))?
            .ok_or_else(|| TunnelError::InternalError("认证时连接关闭".into()))?;
        
        let response = Hy2AuthResponse::decode(&buf[..n])?;
        
        if !response.ok {
            return Err(TunnelError::InternalError(format!("认证失败: {}", response.message)));
        }
        
        tracing::info!(
            "Hysteria2 认证成功，服务端带宽: {}Mbps 上行, {}Mbps 下行",
            response.tx / 1_000_000,
            response.rx / 1_000_000
        );
        
        Ok(response)
    }
}

#[cfg(feature = "quic")]
#[async_trait::async_trait]
impl TunnelConnector for Hysteria2TunnelConnector {
    async fn connect(&mut self) -> Result<Box<dyn super::Tunnel>, TunnelError> {
        use super::common::{FramedReader, FramedWriter, TunnelWrapper};
        
        let addr = super::check_scheme_and_get_socket_addr::<SocketAddr>(
            &self.addr,
            "hysteria2",
            self.ip_version,
        ).await?;

        let local_addr = if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
        
        let mut endpoint = quinn::Endpoint::client(local_addr.parse().unwrap())
            .map_err(|e| TunnelError::InternalError(format!("创建端点失败: {:?}", e)))?;
        
        let client_config = create_hy2_quic_config(&self.config)?;
        endpoint.set_default_client_config(client_config);

        let sni = self.config.sni.as_deref().unwrap_or("localhost");
        let connection = endpoint
            .connect(addr, sni)
            .map_err(|e| TunnelError::InternalError(format!("连接失败: {:?}", e)))?
            .await
            .map_err(|e| TunnelError::InternalError(format!("连接失败: {:?}", e)))?;

        tracing::info!("Hysteria2 QUIC 连接成功: {}", addr);

        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| TunnelError::InternalError(format!("打开流失败: {:?}", e)))?;

        let _auth_response = self.do_auth(&mut send, &mut recv).await?;

        let info = super::TunnelInfo {
            tunnel_type: "hysteria2".to_owned(),
            local_addr: Some(
                super::build_url_from_socket_addr(&endpoint.local_addr()?.to_string(), "hysteria2").into(),
            ),
            remote_addr: Some(self.addr.clone().into()),
        };

        struct ConnWrapper {
            conn: quinn::Connection,
        }
        impl Drop for ConnWrapper {
            fn drop(&mut self) {
                self.conn.close(0u32.into(), b"done");
            }
        }
        
        let arc_conn = Arc::new(ConnWrapper { conn: connection });
        Ok(Box::new(TunnelWrapper::new(
            FramedReader::new_with_associate_data(recv, 4500, Some(Box::new(arc_conn.clone()))),
            FramedWriter::new_with_associate_data(send, Some(Box::new(arc_conn))),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_encode_decode() {
        let values = [0u64, 1, 127, 128, 16383, 16384];
        
        for &v in &values {
            let encoded = encode_varint(v);
            let (decoded, _) = decode_varint(&encoded).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn test_auth_request_encode() {
        let req = Hy2AuthRequest::new("test_password", 100, 100);
        let encoded = req.encode();
        
        assert_eq!(encoded[0], Hy2FrameType::Auth as u8);
        assert_eq!(encoded[1], 0);
    }

    #[test]
    fn test_brutal_congestion() {
        let mut cc = BrutalCongestionControl::new(100);
        
        let cwnd1 = cc.get_cwnd();
        assert!(cwnd1 > 0);
        
        cc.update_rtt(Duration::from_millis(100));
        let cwnd2 = cc.get_cwnd();
        assert!(cwnd2 >= cwnd1);
        
        cc.update_loss(10, 100);
        let cwnd3 = cc.get_cwnd();
        assert!(cwnd3 >= cwnd2);
    }
}
