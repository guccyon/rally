use crate::DomainError;

/// メッセージの本文。空（空白だけ）でなく、[`MessageBody::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageBody(String);

impl MessageBody {
    /// 本文の上限（文字数。student-app・admin-web の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 2000;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyMessageBody);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::MessageBodyTooLong {
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
    fn accepts_non_empty_bodies_up_to_the_limit() {
        assert_eq!(
            MessageBody::new("次回はサーブを見てください")
                .unwrap()
                .as_str(),
            "次回はサーブを見てください"
        );
        assert!(MessageBody::new("あ".repeat(MessageBody::MAX_CHARS)).is_ok());
    }

    #[test]
    fn empty_body_is_a_domain_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(MessageBody::new(s), Err(DomainError::EmptyMessageBody));
        }
    }

    #[test]
    fn too_long_body_is_a_domain_error() {
        assert_eq!(
            MessageBody::new("あ".repeat(MessageBody::MAX_CHARS + 1)),
            Err(DomainError::MessageBodyTooLong {
                max: MessageBody::MAX_CHARS
            })
        );
    }
}
