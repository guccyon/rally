use std::fmt;

use crate::DomainError;

/// アカウントのメールアドレス。
///
/// 形の検査は最小限（`@` が 1 つで前後が空でなく、空白を含まない）。到達確認はしない。
/// 一意性は大文字小文字を区別しない（[`Email::normalized`]。DB の `accounts_email_key` と同じ）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let valid = match value.split_once('@') {
            Some((local, domain)) => {
                !local.is_empty()
                    && !domain.is_empty()
                    && !domain.contains('@')
                    && !value.chars().any(char::is_whitespace)
            }
            None => false,
        };
        if valid {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidEmail)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 一意性の比較に使う形（小文字）。
    pub fn normalized(&self) -> String {
        self.0.to_lowercase()
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_addresses() {
        let email = Email::new("Taro@Example.com").unwrap();
        assert_eq!(email.as_str(), "Taro@Example.com");
        assert_eq!(email.normalized(), "taro@example.com");
    }

    #[test]
    fn malformed_email_is_a_domain_error() {
        for s in [
            "",
            "taro",
            "@example.com",
            "taro@",
            "a@b@c",
            "ta ro@example.com",
        ] {
            assert_eq!(Email::new(s), Err(DomainError::InvalidEmail), "{s:?}");
        }
    }
}
