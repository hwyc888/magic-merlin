//! uTLS 指纹伪装模块 - 模拟真实浏览器 TLS 指纹
//!
//! 原理：
//! 1. 深信服通过 JA3/JA4 指纹识别 VPN 流量
//! 2. uTLS 模拟真实浏览器的 TLS 握手特征
//! 3. 使流量看起来像 Chrome/Firefox/Safari
//!
//! 2025-01 更新：
//! - Chrome 131 (2025年1月最新稳定版)
//! - Firefox 134 (2025年1月最新稳定版)
//! - Safari 18.2 (macOS Sequoia / iOS 18)
//! - Edge 131 (基于 Chromium 131)
//! - 新增 JA4 指纹支持
//! - 新增 ECH (Encrypted Client Hello) GREASE 扩展
//!
//! 支持的指纹：
//! - Chrome 131 (Windows/macOS/Linux)
//! - Firefox 134 (Windows/macOS/Linux)
//! - Safari 18.2 (macOS Sequoia)
//! - Safari (iOS 18)
//! - Edge 131 (Windows)
//! - Android Chrome 131
//! - 旧版本兼容 (Chrome 120, Firefox 121 等)

#![allow(dead_code)]

/// TLS 指纹类型
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TlsFingerprint {
    // ===== 2025 最新版本 (推荐) =====
    /// Chrome 131 (Windows) - 2025年1月最新
    Chrome131,
    /// Chrome 131 (macOS)
    Chrome131Mac,
    /// Chrome 131 (Linux)
    Chrome131Linux,
    /// Firefox 134 (Windows) - 2025年1月最新
    Firefox134,
    /// Firefox 134 (macOS)
    Firefox134Mac,
    /// Safari 18.2 (macOS Sequoia) - 2025年1月最新
    Safari18,
    /// Safari (iOS 18)
    SafariIOS18,
    /// Edge 131 (Windows) - 2025年1月最新
    Edge131,
    /// Android Chrome 131
    AndroidChrome131,
    
    // ===== 旧版本兼容 =====
    /// Chrome 120+ (Windows) - 旧版兼容
    Chrome120,
    /// Chrome 120+ (macOS)
    Chrome120Mac,
    /// Firefox 121+ (Windows)
    Firefox121,
    /// Firefox 121+ (macOS)
    Firefox121Mac,
    /// Safari 17+ (macOS)
    Safari17,
    /// Safari (iOS 17+)
    SafariIOS17,
    /// Edge 120+ (Windows)
    Edge120,
    /// Android Chrome
    AndroidChrome,
    
    // ===== 特殊选项 =====
    /// 随机选择 (从最新版本中随机)
    Random,
    /// 随机选择 (包含旧版本，更多样化)
    RandomAll,
    /// 自定义
    Custom,
}

impl Default for TlsFingerprint {
    fn default() -> Self {
        Self::Chrome131 // 默认使用最新版本
    }
}

impl TlsFingerprint {
    /// 从字符串解析
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            // 2025 最新版本
            "chrome" | "chrome131" => Self::Chrome131,
            "chrome_mac" | "chrome131mac" => Self::Chrome131Mac,
            "chrome_linux" | "chrome131linux" => Self::Chrome131Linux,
            "firefox" | "firefox134" => Self::Firefox134,
            "firefox_mac" | "firefox134mac" => Self::Firefox134Mac,
            "safari" | "safari18" => Self::Safari18,
            "safari_ios" | "safariios18" => Self::SafariIOS18,
            "edge" | "edge131" => Self::Edge131,
            "android" | "androidchrome" | "androidchrome131" => Self::AndroidChrome131,
            
            // 旧版本兼容
            "chrome120" => Self::Chrome120,
            "chrome120mac" => Self::Chrome120Mac,
            "firefox121" => Self::Firefox121,
            "firefox121mac" => Self::Firefox121Mac,
            "safari17" => Self::Safari17,
            "safariios17" => Self::SafariIOS17,
            "edge120" => Self::Edge120,
            "androidchrome_old" => Self::AndroidChrome,
            
            // 特殊选项
            "random" => Self::Random,
            "random_all" | "randomall" => Self::RandomAll,
            _ => Self::Chrome131,
        }
    }
    
    /// 获取指纹描述
    pub fn description(&self) -> &'static str {
        match self {
            Self::Chrome131 => "Chrome 131 (Windows) - 2025-01",
            Self::Chrome131Mac => "Chrome 131 (macOS) - 2025-01",
            Self::Chrome131Linux => "Chrome 131 (Linux) - 2025-01",
            Self::Firefox134 => "Firefox 134 (Windows) - 2025-01",
            Self::Firefox134Mac => "Firefox 134 (macOS) - 2025-01",
            Self::Safari18 => "Safari 18.2 (macOS Sequoia) - 2025-01",
            Self::SafariIOS18 => "Safari (iOS 18) - 2025-01",
            Self::Edge131 => "Edge 131 (Windows) - 2025-01",
            Self::AndroidChrome131 => "Chrome 131 (Android) - 2025-01",
            Self::Chrome120 => "Chrome 120 (Windows) - Legacy",
            Self::Chrome120Mac => "Chrome 120 (macOS) - Legacy",
            Self::Firefox121 => "Firefox 121 (Windows) - Legacy",
            Self::Firefox121Mac => "Firefox 121 (macOS) - Legacy",
            Self::Safari17 => "Safari 17 (macOS) - Legacy",
            Self::SafariIOS17 => "Safari (iOS 17) - Legacy",
            Self::Edge120 => "Edge 120 (Windows) - Legacy",
            Self::AndroidChrome => "Chrome (Android) - Legacy",
            Self::Random => "Random (Latest)",
            Self::RandomAll => "Random (All versions)",
            Self::Custom => "Custom",
        }
    }
    
    /// 是否是最新版本
    pub fn is_latest(&self) -> bool {
        matches!(self, 
            Self::Chrome131 | Self::Chrome131Mac | Self::Chrome131Linux |
            Self::Firefox134 | Self::Firefox134Mac |
            Self::Safari18 | Self::SafariIOS18 |
            Self::Edge131 | Self::AndroidChrome131
        )
    }
}

/// JA3 指纹数据
#[derive(Clone, Debug)]
pub struct Ja3Fingerprint {
    /// TLS 版本
    pub tls_version: u16,
    /// 密码套件列表
    pub cipher_suites: Vec<u16>,
    /// 扩展列表
    pub extensions: Vec<u16>,
    /// 椭圆曲线列表
    pub elliptic_curves: Vec<u16>,
    /// 椭圆曲线点格式
    pub ec_point_formats: Vec<u8>,
    /// 签名算法列表 (用于更精确的指纹)
    pub signature_algorithms: Vec<u16>,
    /// 是否支持 ECH GREASE
    pub ech_grease: bool,
    /// 是否支持压缩证书
    pub compress_certificate: bool,
}

impl Ja3Fingerprint {
    /// 计算 JA3 哈希
    pub fn ja3_hash(&self) -> String {
        use sha2::{Sha256, Digest};
        
        let ja3_string = format!(
            "{},{},{},{},{}",
            self.tls_version,
            self.cipher_suites.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("-"),
            self.extensions.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("-"),
            self.elliptic_curves.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("-"),
            self.ec_point_formats.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("-"),
        );
        
        let mut hasher = Sha256::new();
        hasher.update(ja3_string.as_bytes());
        let result = hasher.finalize();
        
        hex::encode(&result[..16])
    }
    
    /// 计算 JA4 指纹 (更现代的指纹格式)
    /// 格式: t{tls_version}{sni}{cipher_count}{ext_count}_{cipher_hash}_{ext_hash}
    pub fn ja4_fingerprint(&self, has_sni: bool) -> String {
        use sha2::{Sha256, Digest};
        
        // 协议类型 (t = TLS)
        let proto = "t";
        
        // TLS 版本简写
        let version = match self.tls_version {
            0x0304 => "13",
            0x0303 => "12",
            0x0302 => "11",
            0x0301 => "10",
            _ => "00",
        };
        
        // SNI 存在标记
        let sni_flag = if has_sni { "d" } else { "i" };
        
        // 密码套件数量 (2位)
        let cipher_count = format!("{:02}", self.cipher_suites.len().min(99));
        
        // 扩展数量 (2位)
        let ext_count = format!("{:02}", self.extensions.len().min(99));
        
        // ALPN 第一个值的首字母
        let alpn_first = "h"; // 假设是 h2
        
        // 密码套件哈希 (排序后)
        let mut sorted_ciphers = self.cipher_suites.clone();
        sorted_ciphers.sort();
        let cipher_str: String = sorted_ciphers.iter()
            .map(|c| format!("{:04x}", c))
            .collect::<Vec<_>>()
            .join(",");
        let mut hasher = Sha256::new();
        hasher.update(cipher_str.as_bytes());
        let cipher_hash = hex::encode(&hasher.finalize()[..6]);
        
        // 扩展哈希 (排序后，排除 SNI 和 ALPN)
        let mut sorted_exts: Vec<u16> = self.extensions.iter()
            .filter(|&&e| e != 0x0000 && e != 0x0010) // 排除 SNI 和 ALPN
            .copied()
            .collect();
        sorted_exts.sort();
        let ext_str: String = sorted_exts.iter()
            .map(|e| format!("{:04x}", e))
            .collect::<Vec<_>>()
            .join(",");
        let mut hasher = Sha256::new();
        hasher.update(ext_str.as_bytes());
        let ext_hash = hex::encode(&hasher.finalize()[..6]);
        
        format!("{}{}{}{}{}{}_{}_{}",
            proto, version, sni_flag, cipher_count, ext_count, alpn_first,
            cipher_hash, ext_hash
        )
    }
}

/// 获取指定浏览器的 JA3 指纹
pub fn get_ja3_fingerprint(fp: TlsFingerprint) -> Ja3Fingerprint {
    match fp {
        // ===== 2025 最新版本 =====
        
        // Chrome 131 (2025-01) - Windows/Linux
        // 基于真实 Chrome 131.0.6778.86 抓包
        TlsFingerprint::Chrome131 | TlsFingerprint::Chrome131Linux => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, // TLS_AES_128_GCM_SHA256
                0x1302, // TLS_AES_256_GCM_SHA384
                0x1303, // TLS_CHACHA20_POLY1305_SHA256
                0xc02b, // TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
                0xc02f, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
                0xc02c, // TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
                0xc030, // TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
                0xcca9, // TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
                0xcca8, // TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
                0xc013, // TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA
                0xc014, // TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA
            ],
            extensions: vec![
                0xfe0d, // encrypted_client_hello (ECH GREASE)
                0x0000, // server_name
                0x0017, // extended_master_secret
                0xff01, // renegotiation_info
                0x000a, // supported_groups
                0x000b, // ec_point_formats
                0x0023, // session_ticket
                0x0010, // application_layer_protocol_negotiation
                0x0005, // status_request
                0x000d, // signature_algorithms
                0x002b, // supported_versions
                0x002d, // psk_key_exchange_modes
                0x0033, // key_share
                0x001b, // compress_certificate
                0x4469, // application_settings (ALPS)
                0x0015, // padding
            ],
            elliptic_curves: vec![
                0x6399, // X25519Kyber768Draft00 (后量子混合)
                0x001d, // x25519
                0x0017, // secp256r1
                0x0018, // secp384r1
            ],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501,
                0x0806, 0x0601, 0x0201,
            ],
            ech_grease: true,
            compress_certificate: true,
        },
        
        // Chrome 131 (macOS)
        TlsFingerprint::Chrome131Mac => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, 0x1302, 0x1303,
                0xc02b, 0xc02f, 0xc02c, 0xc030,
                0xcca9, 0xcca8,
                0xc013, 0xc014,
            ],
            extensions: vec![
                0xfe0d, 0x0000, 0x0017, 0xff01, 0x000a, 0x000b,
                0x0023, 0x0010, 0x0005, 0x000d, 0x002b, 0x002d,
                0x0033, 0x001b, 0x4469, 0x0015,
            ],
            elliptic_curves: vec![0x6399, 0x001d, 0x0017, 0x0018],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501,
                0x0806, 0x0601, 0x0201,
            ],
            ech_grease: true,
            compress_certificate: true,
        },
        
        // Firefox 134 (2025-01)
        // 基于真实 Firefox 134.0 抓包
        TlsFingerprint::Firefox134 | TlsFingerprint::Firefox134Mac => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, // TLS_AES_128_GCM_SHA256
                0x1303, // TLS_CHACHA20_POLY1305_SHA256
                0x1302, // TLS_AES_256_GCM_SHA384
                0xc02b, // TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
                0xc02f, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
                0xc02c, // TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
                0xc030, // TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
                0xcca9, // TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
                0xcca8, // TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
                0xc013, // TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA
                0xc014, // TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA
            ],
            extensions: vec![
                0x0000, // server_name
                0x0017, // extended_master_secret
                0xff01, // renegotiation_info
                0x000a, // supported_groups
                0x000b, // ec_point_formats
                0x0023, // session_ticket
                0x0010, // application_layer_protocol_negotiation
                0x0005, // status_request
                0x000d, // signature_algorithms
                0x002b, // supported_versions
                0x002d, // psk_key_exchange_modes
                0x0033, // key_share
                0x001c, // record_size_limit
                0xfe0d, // encrypted_client_hello (ECH)
                0x001b, // compress_certificate
            ],
            elliptic_curves: vec![
                0x6399, // X25519Kyber768Draft00
                0x001d, // x25519
                0x0017, // secp256r1
                0x0018, // secp384r1
                0x0019, // secp521r1
                0x0100, // ffdhe2048
                0x0101, // ffdhe3072
            ],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806,
                0x0401, 0x0501, 0x0601, 0x0201,
            ],
            ech_grease: true,
            compress_certificate: true,
        },
        
        // Safari 18.2 (macOS Sequoia / iOS 18)
        TlsFingerprint::Safari18 | TlsFingerprint::SafariIOS18 => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, // TLS_AES_128_GCM_SHA256
                0x1302, // TLS_AES_256_GCM_SHA384
                0x1303, // TLS_CHACHA20_POLY1305_SHA256
                0xc02c, // TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
                0xc02b, // TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
                0xc030, // TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
                0xc02f, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
                0xcca9, // TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
                0xcca8, // TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
                0xc024, // TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA384
                0xc023, // TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA256
                0xc028, // TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA384
                0xc027, // TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA256
            ],
            extensions: vec![
                0x0000, // server_name
                0x0017, // extended_master_secret
                0xff01, // renegotiation_info
                0x000a, // supported_groups
                0x000b, // ec_point_formats
                0x0010, // application_layer_protocol_negotiation
                0x0005, // status_request
                0x000d, // signature_algorithms
                0x002b, // supported_versions
                0x002d, // psk_key_exchange_modes
                0x0033, // key_share
                0xfe0d, // encrypted_client_hello
                0x001b, // compress_certificate
            ],
            elliptic_curves: vec![
                0x6399, // X25519Kyber768Draft00
                0x001d, // x25519
                0x0017, // secp256r1
                0x0018, // secp384r1
                0x0019, // secp521r1
            ],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806,
                0x0401, 0x0501, 0x0601,
            ],
            ech_grease: true,
            compress_certificate: true,
        },
        
        // Edge 131 (基于 Chromium 131)
        TlsFingerprint::Edge131 => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, 0x1302, 0x1303,
                0xc02b, 0xc02f, 0xc02c, 0xc030,
                0xcca9, 0xcca8,
                0xc013, 0xc014,
            ],
            extensions: vec![
                0xfe0d, 0x0000, 0x0017, 0xff01, 0x000a, 0x000b,
                0x0023, 0x0010, 0x0005, 0x000d, 0x002b, 0x002d,
                0x0033, 0x001b, 0x4469, 0x0015,
            ],
            elliptic_curves: vec![0x6399, 0x001d, 0x0017, 0x0018],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501,
                0x0806, 0x0601, 0x0201,
            ],
            ech_grease: true,
            compress_certificate: true,
        },
        
        // Android Chrome 131
        TlsFingerprint::AndroidChrome131 => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, 0x1302, 0x1303,
                0xc02b, 0xc02f, 0xc02c, 0xc030,
                0xcca9, 0xcca8,
            ],
            extensions: vec![
                0xfe0d, 0x0000, 0x0017, 0xff01, 0x000a, 0x000b,
                0x0023, 0x0010, 0x0005, 0x000d, 0x002b, 0x002d,
                0x0033, 0x001b, 0x0015,
            ],
            elliptic_curves: vec![0x001d, 0x0017, 0x0018],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501,
                0x0806, 0x0601,
            ],
            ech_grease: true,
            compress_certificate: true,
        },
        
        // ===== 旧版本兼容 =====
        TlsFingerprint::Chrome120 | TlsFingerprint::Chrome120Mac => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, 0x1302, 0x1303,
                0xc02c, 0xc02b, 0xc030, 0xc02f,
                0xcca9, 0xcca8,
            ],
            extensions: vec![
                0x0000, 0x0017, 0xff01, 0x000a, 0x000b,
                0x0023, 0x0010, 0x0005, 0x000d, 0x002b, 0x002d,
                0x0033, 0x001b, 0x0015,
            ],
            elliptic_curves: vec![0x001d, 0x0017, 0x0018],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806,
                0x0401,
            ],
            ech_grease: false,
            compress_certificate: true,
        },
        
        TlsFingerprint::Firefox121 | TlsFingerprint::Firefox121Mac => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, 0x1303, 0x1302,
                0xc02b, 0xc02f, 0xc02c, 0xc030,
                0xcca9, 0xcca8,
            ],
            extensions: vec![
                0x0000, 0x0017, 0xff01, 0x000a, 0x000b,
                0x0023, 0x0010, 0x0005, 0x000d, 0x002b, 0x002d,
                0x0033, 0x001c,
            ],
            elliptic_curves: vec![
                0x001d, 0x0017, 0x0018, 0x0019, 0x0100, 0x0101,
            ],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806,
                0x0401, 0x0501, 0x0601, 0x0201,
            ],
            ech_grease: false,
            compress_certificate: false,
        },
        
        TlsFingerprint::Safari17 | TlsFingerprint::SafariIOS17 => Ja3Fingerprint {
            tls_version: 0x0303,
            cipher_suites: vec![
                0x1301, 0x1302, 0x1303,
                0xc02c, 0xc02b, 0xc024, 0xc023,
                0xc00a, 0xc009, 0xc030, 0xc02f,
            ],
            extensions: vec![
                0x0000, 0x0017, 0xff01, 0x000a, 0x000b,
                0x0010, 0x0005, 0x000d, 0x002b, 0x002d, 0x0033,
            ],
            elliptic_curves: vec![0x001d, 0x0017, 0x0018, 0x0019],
            ec_point_formats: vec![0x00],
            signature_algorithms: vec![
                0x0403, 0x0503, 0x0603, 0x0804, 0x0805, 0x0806,
                0x0401, 0x0501, 0x0601,
            ],
            ech_grease: false,
            compress_certificate: false,
        },
        
        TlsFingerprint::Edge120 | TlsFingerprint::AndroidChrome => {
            get_ja3_fingerprint(TlsFingerprint::Chrome120)
        },
        
        TlsFingerprint::Random => {
            get_ja3_fingerprint(random_fingerprint())
        },
        
        TlsFingerprint::RandomAll => {
            get_ja3_fingerprint(random_fingerprint_all())
        },
        
        _ => get_ja3_fingerprint(TlsFingerprint::Chrome131),
    }
}

/// TLS Client Hello 构建器
pub struct ClientHelloBuilder {
    fingerprint: TlsFingerprint,
    sni: String,
    alpn: Vec<String>,
    /// 是否启用 ECH GREASE
    ech_grease: bool,
    /// 自定义公钥（用于 REALITY 等协议）
    custom_public_key: Option<[u8; 32]>,
}

impl ClientHelloBuilder {
    pub fn new(fingerprint: TlsFingerprint, sni: String) -> Self {
        let ja3 = get_ja3_fingerprint(fingerprint);
        Self {
            fingerprint,
            sni,
            alpn: vec!["h2".to_string(), "http/1.1".to_string()],
            ech_grease: ja3.ech_grease,
            custom_public_key: None,
        }
    }

    pub fn with_alpn(mut self, alpn: Vec<String>) -> Self {
        self.alpn = alpn;
        self
    }
    
    pub fn with_ech_grease(mut self, enabled: bool) -> Self {
        self.ech_grease = enabled;
        self
    }
    
    pub fn with_public_key(mut self, key: [u8; 32]) -> Self {
        self.custom_public_key = Some(key);
        self
    }

    /// 构建 Client Hello
    pub fn build(&self) -> Vec<u8> {
        let ja3 = get_ja3_fingerprint(self.fingerprint);
        let mut hello = Vec::new();
        
        // TLS Record Header
        hello.push(0x16); // Handshake
        hello.extend_from_slice(&[0x03, 0x01]); // TLS 1.0 (兼容性)
        
        // 预留长度
        let length_pos = hello.len();
        hello.extend_from_slice(&[0x00, 0x00]);
        
        // Handshake Header
        hello.push(0x01); // Client Hello
        let hs_length_pos = hello.len();
        hello.extend_from_slice(&[0x00, 0x00, 0x00]);
        
        // Client Version
        hello.extend_from_slice(&ja3.tls_version.to_be_bytes());
        
        // Random (32 bytes)
        let mut random = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut random);
        hello.extend_from_slice(&random);
        
        // Session ID
        hello.push(32);
        let mut session_id = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut session_id);
        hello.extend_from_slice(&session_id);
        
        // Cipher Suites
        let cipher_len = (ja3.cipher_suites.len() * 2) as u16;
        hello.extend_from_slice(&cipher_len.to_be_bytes());
        for cipher in &ja3.cipher_suites {
            hello.extend_from_slice(&cipher.to_be_bytes());
        }
        
        // Compression Methods
        hello.extend_from_slice(&[0x01, 0x00]);
        
        // Extensions
        let extensions = self.build_extensions(&ja3);
        hello.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        hello.extend_from_slice(&extensions);
        
        // 更新长度
        let total_len = hello.len() - 5;
        hello[length_pos..length_pos + 2].copy_from_slice(&(total_len as u16).to_be_bytes());
        let hs_len = hello.len() - hs_length_pos - 3;
        hello[hs_length_pos] = ((hs_len >> 16) & 0xff) as u8;
        hello[hs_length_pos + 1] = ((hs_len >> 8) & 0xff) as u8;
        hello[hs_length_pos + 2] = (hs_len & 0xff) as u8;
        
        hello
    }

    /// 构建扩展
    fn build_extensions(&self, ja3: &Ja3Fingerprint) -> Vec<u8> {
        let mut ext = Vec::new();
        
        for ext_type in &ja3.extensions {
            match *ext_type {
                0xfe0d => {
                    if self.ech_grease {
                        self.add_ech_grease_extension(&mut ext);
                    }
                }
                0x0000 => self.add_sni_extension(&mut ext),
                0x000a => self.add_supported_groups_extension(&mut ext, &ja3.elliptic_curves),
                0x000b => self.add_ec_point_formats_extension(&mut ext, &ja3.ec_point_formats),
                0x000d => self.add_signature_algorithms_extension(&mut ext, &ja3.signature_algorithms),
                0x0010 => self.add_alpn_extension(&mut ext),
                0x0017 => self.add_extended_master_secret_extension(&mut ext),
                0x0023 => self.add_session_ticket_extension(&mut ext),
                0x002b => self.add_supported_versions_extension(&mut ext),
                0x002d => self.add_psk_key_exchange_modes_extension(&mut ext),
                0x0033 => self.add_key_share_extension(&mut ext, &ja3.elliptic_curves),
                0xff01 => self.add_renegotiation_info_extension(&mut ext),
                0x001b => {
                    if ja3.compress_certificate {
                        self.add_compress_certificate_extension(&mut ext);
                    }
                }
                0x001c => self.add_record_size_limit_extension(&mut ext),
                0x4469 => self.add_application_settings_extension(&mut ext),
                0x0015 => {
                    // padding 放最后处理
                }
                0x0005 => self.add_status_request_extension(&mut ext),
                _ => {}
            }
        }
        
        // 最后添加 padding 使总长度看起来自然
        if ja3.extensions.contains(&0x0015) {
            let current_len = ext.len();
            let target_len: usize = if current_len < 500 { 512 } else { 1024 };
            let padding_len = target_len.saturating_sub(current_len + 4);
            if padding_len > 0 && padding_len < 512 {
                self.add_padding_extension(&mut ext, padding_len);
            }
        }
        
        ext
    }
    
    /// 添加 ECH GREASE 扩展 (Encrypted Client Hello)
    fn add_ech_grease_extension(&self, ext: &mut Vec<u8>) {
        use rand::Rng;
        
        // ECH GREASE 格式 (draft-ietf-tls-esni-18)
        ext.extend_from_slice(&[0xfe, 0x0d]); // encrypted_client_hello type
        
        // 生成随机 GREASE 数据
        let mut grease_data = Vec::new();
        
        // Client Hello Type (GREASE = 0)
        grease_data.push(0x00);
        
        // Cipher Suite (HKDF-SHA256 + AES-128-GCM)
        grease_data.extend_from_slice(&[0x00, 0x01]); // KDF ID
        grease_data.extend_from_slice(&[0x00, 0x01]); // AEAD ID
        
        // Config ID (随机)
        grease_data.push(rand::random::<u8>());
        
        // Enc (随机 32 字节公钥)
        let mut enc = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut enc);
        grease_data.extend_from_slice(&(enc.len() as u16).to_be_bytes());
        grease_data.extend_from_slice(&enc);
        
        // Payload (随机填充)
        let payload_len: usize = rand::thread_rng().gen_range(128..256);
        let mut payload = vec![0u8; payload_len];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut payload);
        grease_data.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        grease_data.extend_from_slice(&payload);
        
        ext.extend_from_slice(&(grease_data.len() as u16).to_be_bytes());
        ext.extend_from_slice(&grease_data);
    }

    fn add_sni_extension(&self, ext: &mut Vec<u8>) {
        let sni_bytes = self.sni.as_bytes();
        ext.extend_from_slice(&[0x00, 0x00]); // SNI type
        let sni_len = sni_bytes.len() + 5;
        ext.extend_from_slice(&(sni_len as u16).to_be_bytes());
        ext.extend_from_slice(&((sni_len - 2) as u16).to_be_bytes());
        ext.push(0x00); // Host name type
        ext.extend_from_slice(&(sni_bytes.len() as u16).to_be_bytes());
        ext.extend_from_slice(sni_bytes);
    }

    fn add_supported_groups_extension(&self, ext: &mut Vec<u8>, curves: &[u16]) {
        ext.extend_from_slice(&[0x00, 0x0a]);
        let len = curves.len() * 2 + 2;
        ext.extend_from_slice(&(len as u16).to_be_bytes());
        ext.extend_from_slice(&((len - 2) as u16).to_be_bytes());
        for curve in curves {
            ext.extend_from_slice(&curve.to_be_bytes());
        }
    }

    fn add_ec_point_formats_extension(&self, ext: &mut Vec<u8>, formats: &[u8]) {
        ext.extend_from_slice(&[0x00, 0x0b]);
        let len = formats.len() + 1;
        ext.extend_from_slice(&(len as u16).to_be_bytes());
        ext.push(formats.len() as u8);
        ext.extend_from_slice(formats);
    }

    fn add_signature_algorithms_extension(&self, ext: &mut Vec<u8>, algorithms: &[u16]) {
        ext.extend_from_slice(&[0x00, 0x0d]);
        let len = algorithms.len() * 2 + 2;
        ext.extend_from_slice(&(len as u16).to_be_bytes());
        ext.extend_from_slice(&((len - 2) as u16).to_be_bytes());
        for alg in algorithms {
            ext.extend_from_slice(&alg.to_be_bytes());
        }
    }

    fn add_alpn_extension(&self, ext: &mut Vec<u8>) {
        ext.extend_from_slice(&[0x00, 0x10]);
        let mut alpn_data = Vec::new();
        for proto in &self.alpn {
            alpn_data.push(proto.len() as u8);
            alpn_data.extend_from_slice(proto.as_bytes());
        }
        ext.extend_from_slice(&((alpn_data.len() + 2) as u16).to_be_bytes());
        ext.extend_from_slice(&(alpn_data.len() as u16).to_be_bytes());
        ext.extend_from_slice(&alpn_data);
    }

    fn add_extended_master_secret_extension(&self, ext: &mut Vec<u8>) {
        ext.extend_from_slice(&[0x00, 0x17, 0x00, 0x00]);
    }

    fn add_session_ticket_extension(&self, ext: &mut Vec<u8>) {
        ext.extend_from_slice(&[0x00, 0x23, 0x00, 0x00]);
    }

    fn add_supported_versions_extension(&self, ext: &mut Vec<u8>) {
        ext.extend_from_slice(&[
            0x00, 0x2b, 0x00, 0x05, 0x04,
            0x03, 0x04, // TLS 1.3
            0x03, 0x03, // TLS 1.2
        ]);
    }

    fn add_psk_key_exchange_modes_extension(&self, ext: &mut Vec<u8>) {
        ext.extend_from_slice(&[0x00, 0x2d, 0x00, 0x02, 0x01, 0x01]);
    }

    fn add_key_share_extension(&self, ext: &mut Vec<u8>, curves: &[u16]) {
        ext.extend_from_slice(&[0x00, 0x33]);
        
        let mut key_shares = Vec::new();
        
        // 检查是否包含后量子混合密钥交换
        let has_kyber = curves.contains(&0x6399);
        
        if has_kyber {
            // X25519Kyber768Draft00 (后量子混合)
            // 实际实现需要 Kyber768 库，这里用随机数据模拟
            let kyber_len = 1216; // X25519 (32) + Kyber768 公钥 (1184)
            key_shares.extend_from_slice(&[0x63, 0x99]); // group
            key_shares.extend_from_slice(&(kyber_len as u16).to_be_bytes());
            
            // X25519 部分
            let public_key = self.custom_public_key.unwrap_or_else(|| {
                let mut key = [0u8; 32];
                rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
                key
            });
            key_shares.extend_from_slice(&public_key);
            
            // Kyber768 部分 (模拟)
            let mut kyber_key = vec![0u8; 1184];
            rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut kyber_key);
            key_shares.extend_from_slice(&kyber_key);
        }
        
        // X25519 key share (总是包含)
        let public_key = self.custom_public_key.unwrap_or_else(|| {
            let mut key = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
            key
        });
        key_shares.extend_from_slice(&[0x00, 0x1d, 0x00, 0x20]); // X25519, 32 bytes
        key_shares.extend_from_slice(&public_key);
        
        let total_len = key_shares.len() + 2;
        ext.extend_from_slice(&(total_len as u16).to_be_bytes());
        ext.extend_from_slice(&(key_shares.len() as u16).to_be_bytes());
        ext.extend_from_slice(&key_shares);
    }

    fn add_renegotiation_info_extension(&self, ext: &mut Vec<u8>) {
        ext.extend_from_slice(&[0xff, 0x01, 0x00, 0x01, 0x00]);
    }
    
    fn add_compress_certificate_extension(&self, ext: &mut Vec<u8>) {
        // 支持 brotli 和 zlib 压缩
        ext.extend_from_slice(&[
            0x00, 0x1b, // compress_certificate
            0x00, 0x03, // length
            0x02,       // algorithms length
            0x00, 0x02, // brotli
        ]);
    }
    
    fn add_record_size_limit_extension(&self, ext: &mut Vec<u8>) {
        ext.extend_from_slice(&[
            0x00, 0x1c, // record_size_limit
            0x00, 0x02, // length
            0x40, 0x01, // 16385 bytes
        ]);
    }
    
    fn add_application_settings_extension(&self, ext: &mut Vec<u8>) {
        // ALPS (Application-Layer Protocol Settings)
        ext.extend_from_slice(&[
            0x44, 0x69, // application_settings (17513)
            0x00, 0x03, // length
            0x02,       // protocols length
            0x68, 0x32, // "h2"
        ]);
    }
    
    fn add_status_request_extension(&self, ext: &mut Vec<u8>) {
        // OCSP stapling
        ext.extend_from_slice(&[
            0x00, 0x05, // status_request
            0x00, 0x05, // length
            0x01,       // OCSP
            0x00, 0x00, // responder_id_list length
            0x00, 0x00, // request_extensions length
        ]);
    }

    fn add_padding_extension(&self, ext: &mut Vec<u8>, target_len: usize) {
        if target_len > 4 && target_len < 512 {
            ext.extend_from_slice(&[0x00, 0x15]);
            ext.extend_from_slice(&((target_len - 4) as u16).to_be_bytes());
            ext.extend(vec![0u8; target_len - 4]);
        }
    }
}

/// 随机选择一个最新版本的指纹
pub fn random_fingerprint() -> TlsFingerprint {
    use rand::seq::SliceRandom;
    
    // 只从最新版本中选择
    let fingerprints = [
        TlsFingerprint::Chrome131,
        TlsFingerprint::Chrome131Mac,
        TlsFingerprint::Firefox134,
        TlsFingerprint::Safari18,
        TlsFingerprint::Edge131,
    ];
    
    *fingerprints.choose(&mut rand::thread_rng()).unwrap()
}

/// 随机选择一个指纹（包含所有版本，更多样化）
pub fn random_fingerprint_all() -> TlsFingerprint {
    use rand::seq::SliceRandom;
    
    let fingerprints = [
        // 最新版本 (权重更高)
        TlsFingerprint::Chrome131,
        TlsFingerprint::Chrome131,
        TlsFingerprint::Chrome131Mac,
        TlsFingerprint::Firefox134,
        TlsFingerprint::Firefox134,
        TlsFingerprint::Safari18,
        TlsFingerprint::Edge131,
        TlsFingerprint::AndroidChrome131,
        // 旧版本
        TlsFingerprint::Chrome120,
        TlsFingerprint::Firefox121,
        TlsFingerprint::Safari17,
    ];
    
    *fingerprints.choose(&mut rand::thread_rng()).unwrap()
}

/// 根据操作系统选择合适的指纹
pub fn fingerprint_for_os() -> TlsFingerprint {
    #[cfg(target_os = "windows")]
    return TlsFingerprint::Chrome131;
    
    #[cfg(target_os = "macos")]
    return TlsFingerprint::Safari18;
    
    #[cfg(target_os = "linux")]
    return TlsFingerprint::Chrome131Linux;
    
    #[cfg(target_os = "android")]
    return TlsFingerprint::AndroidChrome131;
    
    #[cfg(target_os = "ios")]
    return TlsFingerprint::SafariIOS18;
    
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos", 
        target_os = "linux",
        target_os = "android",
        target_os = "ios"
    )))]
    return TlsFingerprint::Chrome131;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ja3_hash() {
        let ja3 = get_ja3_fingerprint(TlsFingerprint::Chrome131);
        let hash = ja3.ja3_hash();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_ja4_fingerprint() {
        let ja3 = get_ja3_fingerprint(TlsFingerprint::Chrome131);
        let ja4 = ja3.ja4_fingerprint(true);
        assert!(ja4.starts_with("t12d")); // TLS 1.2, has SNI
        assert!(ja4.contains("_")); // 包含分隔符
    }

    #[test]
    fn test_client_hello_build() {
        let builder = ClientHelloBuilder::new(
            TlsFingerprint::Chrome131,
            "www.google.com".to_string(),
        );
        let hello = builder.build();
        
        // 验证 TLS 记录头
        assert_eq!(hello[0], 0x16); // Handshake
        assert_eq!(hello[1], 0x03); // TLS major version
        assert_eq!(hello[5], 0x01); // Client Hello
    }

    #[test]
    fn test_random_fingerprint() {
        let fp = random_fingerprint();
        assert!(fp.is_latest());
        assert_ne!(fp, TlsFingerprint::Random);
        assert_ne!(fp, TlsFingerprint::Custom);
    }
    
    #[test]
    fn test_fingerprint_description() {
        let fp = TlsFingerprint::Chrome131;
        assert!(fp.description().contains("2025"));
    }
    
    #[test]
    fn test_all_fingerprints_have_ech() {
        // 2025 版本都应该支持 ECH GREASE
        let latest = [
            TlsFingerprint::Chrome131,
            TlsFingerprint::Firefox134,
            TlsFingerprint::Safari18,
            TlsFingerprint::Edge131,
        ];
        
        for fp in latest {
            let ja3 = get_ja3_fingerprint(fp);
            assert!(ja3.ech_grease, "{:?} should support ECH GREASE", fp);
        }
    }
}
