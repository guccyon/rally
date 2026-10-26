use std::fmt;

use crate::DomainError;

/// アカウントの表示名。空（空白だけ）でなく、[`DisplayName::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayName(String);

impl DisplayName {
    /// 表示名の上限（文字数）。
    pub const MAX_CHARS: usize = 50;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyDisplayName);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::DisplayNameTooLong {
                max: Self::MAX_CHARS,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DisplayName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_non_empty_names_up_to_the_limit() {
        assert_eq!(DisplayName::new("たろう").unwrap().as_str(), "たろう");
        assert!(DisplayName::new("あ".repeat(DisplayName::MAX_CHARS)).is_ok());
    }

    #[test]
    fn empty_display_name_is_a_domain_error() {
        for s in ["", " ", "\u{3000}\t"] {
            assert_eq!(DisplayName::new(s), Err(DomainError::EmptyDisplayName));
        }
    }

    #[test]
    fn too_long_display_name_is_a_domain_error() {
        assert_eq!(
            DisplayName::new("あ".repeat(DisplayName::MAX_CHARS + 1)),
            Err(DomainError::DisplayNameTooLong {
                max: DisplayName::MAX_CHARS
            })
        );
    }
}
