use std::fmt;

use crate::DomainError;

/// コーチの自己紹介文。空でもよく、[`CoachBio::MAX_CHARS`] 文字まで。
///
/// 書いたとおりに持つ（前後の空白も削らない）。登録したばかりのコーチは空（[`CoachBio::default`]）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoachBio(String);

impl CoachBio {
    /// 自己紹介文の上限（文字数。Unicode のスカラー値で数える）。
    pub const MAX_CHARS: usize = 400;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::CoachBioTooLong {
                max: Self::MAX_CHARS,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CoachBio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_empty_and_texts_up_to_the_limit() {
        assert_eq!(CoachBio::new("").unwrap(), CoachBio::default());
        assert_eq!(CoachBio::new(" ").unwrap().as_str(), " ");
        assert_eq!(
            CoachBio::new("テニス歴 20 年").unwrap().as_str(),
            "テニス歴 20 年"
        );
        assert!(CoachBio::new("あ".repeat(CoachBio::MAX_CHARS)).is_ok());
    }

    #[test]
    fn too_long_bio_is_a_domain_error() {
        assert_eq!(
            CoachBio::new("あ".repeat(CoachBio::MAX_CHARS + 1)),
            Err(DomainError::CoachBioTooLong {
                max: CoachBio::MAX_CHARS
            })
        );
    }

    #[test]
    fn displays_the_bio_as_given() {
        assert_eq!(CoachBio::new("よろしく").unwrap().to_string(), "よろしく");
    }
}
