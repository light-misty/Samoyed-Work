//! URL 验证器：确保 URL 安全合法
//! 防止访问内网地址、恶意 URL、非 HTTP(S) 协议

use url::Url;

/// URL 验证结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// 验证通过
    Valid,
    /// 验证失败，包含原因
    Invalid(String),
}

/// URL 验证器
pub struct UrlValidator {
    /// 允许的协议（默认仅 https、http）
    allowed_schemes: Vec<String>,
    /// 禁止的主机（内网地址等）
    blocked_hosts: Vec<String>,
    /// 最大 URL 长度
    max_url_length: usize,
}

impl Default for UrlValidator {
    fn default() -> Self {
        Self {
            allowed_schemes: vec!["https".to_string(), "http".to_string()],
            blocked_hosts: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "0.0.0.0".to_string(),
                "::1".to_string(),
                "[::1]".to_string(), // IPv6 回环（url crate host_str 含方括号）
                "[0:0:0:0:0:0:0:1]".to_string(), // IPv6 回环完整形式
                "[fe80:".to_string(), // IPv6 链路本地
                "[fc".to_string(),   // IPv6 唯一本地地址 fc00::/7
                "[fd".to_string(),   // IPv6 唯一本地地址 fd00::/8
                "169.254.".to_string(), // 链路本地
                "10.".to_string(),   // 内网 10.0.0.0/8
                "172.16.".to_string(), // 内网 172.16.0.0/12（简化匹配，仅匹配 172.16. 前缀）
                "172.17.".to_string(),
                "172.18.".to_string(),
                "172.19.".to_string(),
                "172.20.".to_string(),
                "172.21.".to_string(),
                "172.22.".to_string(),
                "172.23.".to_string(),
                "172.24.".to_string(),
                "172.25.".to_string(),
                "172.26.".to_string(),
                "172.27.".to_string(),
                "172.28.".to_string(),
                "172.29.".to_string(),
                "172.30.".to_string(),
                "172.31.".to_string(),
                "192.168.".to_string(), // 内网 192.168.0.0/16
            ],
            max_url_length: 2048,
        }
    }
}

impl UrlValidator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 验证 URL
    pub fn validate(&self, url_str: &str) -> ValidationResult {
        // 检查 URL 长度
        if url_str.len() > self.max_url_length {
            return ValidationResult::Invalid(format!(
                "URL 长度超过限制({} 字符)",
                self.max_url_length
            ));
        }

        // 解析 URL
        let url = match Url::parse(url_str) {
            Ok(u) => u,
            Err(e) => return ValidationResult::Invalid(format!("URL 解析失败: {}", e)),
        };

        // 检查协议
        let scheme = url.scheme();
        if !self.allowed_schemes.iter().any(|s| s == scheme) {
            return ValidationResult::Invalid(format!(
                "不允许的协议: {}(仅允许 {:?})",
                scheme, self.allowed_schemes
            ));
        }

        // 检查主机
        let host = url.host_str().unwrap_or("");
        for blocked in &self.blocked_hosts {
            // 精确匹配或前缀匹配（用于网段）
            if host == blocked || host.starts_with(blocked.as_str()) {
                return ValidationResult::Invalid(format!(
                    "禁止访问的主机: {}(内网/本地地址)",
                    host
                ));
            }
        }

        // 检查端口（禁止常见内部服务端口）
        if let Some(port) = url.port() {
            if matches!(
                port,
                22 | 23 | 25 | 110 | 143 | 3306 | 5432 | 6379 | 8080 | 9200
            ) {
                return ValidationResult::Invalid(format!(
                    "禁止访问的端口: {}(内部服务端口)",
                    port
                ));
            }
        }

        ValidationResult::Valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 localhost 被拒绝
    #[test]
    fn test_rejects_localhost() {
        let validator = UrlValidator::new();
        let result = validator.validate("http://localhost/path");
        assert!(matches!(result, ValidationResult::Invalid(_)));
        if let ValidationResult::Invalid(msg) = result {
            assert!(msg.contains("localhost"), "消息应包含 localhost: {}", msg);
        }
    }

    /// 验证 127.0.0.1 被拒绝
    #[test]
    fn test_rejects_127_0_0_1() {
        let validator = UrlValidator::new();
        let result = validator.validate("http://127.0.0.1/path");
        assert!(matches!(result, ValidationResult::Invalid(_)));
        if let ValidationResult::Invalid(msg) = result {
            assert!(msg.contains("127.0.0.1"), "消息应包含 127.0.0.1: {}", msg);
        }
    }

    /// 验证 192.168.1.1 被拒绝
    #[test]
    fn test_rejects_192_168() {
        let validator = UrlValidator::new();
        let result = validator.validate("http://192.168.1.1/path");
        assert!(matches!(result, ValidationResult::Invalid(_)));
        if let ValidationResult::Invalid(msg) = result {
            assert!(
                msg.contains("192.168.1.1"),
                "消息应包含 192.168.1.1: {}",
                msg
            );
        }
    }

    /// 验证 10.0.0.1 被拒绝
    #[test]
    fn test_rejects_10() {
        let validator = UrlValidator::new();
        let result = validator.validate("http://10.0.0.1/path");
        assert!(matches!(result, ValidationResult::Invalid(_)));
        if let ValidationResult::Invalid(msg) = result {
            assert!(msg.contains("10.0.0.1"), "消息应包含 10.0.0.1: {}", msg);
        }
    }

    /// 验证 https://example.com 被接受
    #[test]
    fn test_accepts_example_com() {
        let validator = UrlValidator::new();
        let result = validator.validate("https://example.com/path");
        assert_eq!(result, ValidationResult::Valid);
    }

    /// 验证 file:///etc/passwd 被拒绝
    #[test]
    fn test_rejects_file_protocol() {
        let validator = UrlValidator::new();
        let result = validator.validate("file:///etc/passwd");
        assert!(matches!(result, ValidationResult::Invalid(_)));
        if let ValidationResult::Invalid(msg) = result {
            assert!(msg.contains("file"), "消息应包含协议名 file: {}", msg);
        }
    }

    /// 验证 ftp://example.com 被拒绝
    #[test]
    fn test_rejects_ftp_protocol() {
        let validator = UrlValidator::new();
        let result = validator.validate("ftp://example.com/file");
        assert!(matches!(result, ValidationResult::Invalid(_)));
        if let ValidationResult::Invalid(msg) = result {
            assert!(msg.contains("ftp"), "消息应包含协议名 ftp: {}", msg);
        }
    }

    /// 验证 https://example.com:3306 被拒绝
    #[test]
    fn test_rejects_internal_port() {
        let validator = UrlValidator::new();
        let result = validator.validate("https://example.com:3306/path");
        assert!(matches!(result, ValidationResult::Invalid(_)));
        if let ValidationResult::Invalid(msg) = result {
            assert!(msg.contains("3306"), "消息应包含端口号 3306: {}", msg);
        }
    }
}
