use crate::DomainError;

/// レッスンメニューの名前。空（空白だけ）でなく、[`LessonMenuName::MAX_CHARS`] 文字まで。書いたとおりに持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonMenuName(String);

impl LessonMenuName {
    /// 名前の上限（文字数。Unicode のスカラー値で数える。契約の `maxLength`）。
    pub const MAX_CHARS: usize = 40;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyLessonMenuName);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::LessonMenuNameTooLong {
                max: Self::MAX_CHARS,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_names_from_one_char_up_to_the_limit() {
        assert_eq!(LessonMenuName::new("あ").unwrap().as_str(), "あ");
        assert_eq!(
            LessonMenuName::new("フォア改善 30 分").unwrap().as_str(),
            "フォア改善 30 分"
        );
        assert!(LessonMenuName::new("あ".repeat(LessonMenuName::MAX_CHARS)).is_ok());
    }

    #[test]
    fn empty_name_is_a_domain_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(
                LessonMenuName::new(s),
                Err(DomainError::EmptyLessonMenuName)
            );
        }
    }

    #[test]
    fn too_long_name_is_a_domain_error() {
        assert_eq!(
            LessonMenuName::new("あ".repeat(LessonMenuName::MAX_CHARS + 1)),
            Err(DomainError::LessonMenuNameTooLong {
                max: LessonMenuName::MAX_CHARS
            })
        );
    }
}
