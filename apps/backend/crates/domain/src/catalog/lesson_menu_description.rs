use crate::DomainError;

/// レッスンメニューの説明。空でもよく、[`LessonMenuDescription::MAX_CHARS`] 文字まで。書いたとおりに持つ
/// （前後の空白も削らない）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LessonMenuDescription(String);

impl LessonMenuDescription {
    /// 説明の上限（文字数。Unicode のスカラー値で数える。admin-web の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 400;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::LessonMenuDescriptionTooLong {
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
    fn accepts_empty_and_texts_up_to_the_limit() {
        assert_eq!(
            LessonMenuDescription::new("").unwrap(),
            LessonMenuDescription::default()
        );
        assert_eq!(LessonMenuDescription::new(" ").unwrap().as_str(), " ");
        assert!(LessonMenuDescription::new("あ".repeat(LessonMenuDescription::MAX_CHARS)).is_ok());
    }

    #[test]
    fn too_long_description_is_a_domain_error() {
        assert_eq!(
            LessonMenuDescription::new("あ".repeat(LessonMenuDescription::MAX_CHARS + 1)),
            Err(DomainError::LessonMenuDescriptionTooLong {
                max: LessonMenuDescription::MAX_CHARS
            })
        );
    }
}
