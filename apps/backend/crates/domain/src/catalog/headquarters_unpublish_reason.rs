use crate::DomainError;

/// 本部がレッスンメニューを非公開にしたときに書く理由。空でもよく（書かなかった）、
/// [`HeadquartersUnpublishReason::MAX_CHARS`] 文字まで。書いたとおりに持つ（前後の空白も削らない）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeadquartersUnpublishReason(String);

impl HeadquartersUnpublishReason {
    /// 理由の上限（文字数。Unicode のスカラー値で数える。admin-web の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 200;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::HeadquartersUnpublishReasonTooLong {
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
            HeadquartersUnpublishReason::new("").unwrap(),
            HeadquartersUnpublishReason::default()
        );
        assert_eq!(
            HeadquartersUnpublishReason::new(" 誤解を招く表現 ")
                .unwrap()
                .as_str(),
            " 誤解を招く表現 "
        );
        assert!(
            HeadquartersUnpublishReason::new("あ".repeat(HeadquartersUnpublishReason::MAX_CHARS))
                .is_ok()
        );
    }

    #[test]
    fn too_long_reason_is_a_domain_error() {
        assert_eq!(
            HeadquartersUnpublishReason::new(
                "あ".repeat(HeadquartersUnpublishReason::MAX_CHARS + 1)
            ),
            Err(DomainError::HeadquartersUnpublishReasonTooLong {
                max: HeadquartersUnpublishReason::MAX_CHARS
            })
        );
    }
}
