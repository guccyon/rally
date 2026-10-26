use crate::DomainError;

/// コーチの添削コメント。空（空白だけ）でなく、[`ReviewComment::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewComment(String);

impl ReviewComment {
    /// 添削コメントの上限（文字数。admin-web の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 4000;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyReviewComment);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::ReviewCommentTooLong {
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
    fn accepts_non_empty_comments_up_to_the_limit() {
        assert_eq!(
            ReviewComment::new("インパクトで顔が残っている")
                .unwrap()
                .as_str(),
            "インパクトで顔が残っている"
        );
        assert!(ReviewComment::new("あ".repeat(ReviewComment::MAX_CHARS)).is_ok());
    }

    #[test]
    fn empty_comment_is_a_domain_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(ReviewComment::new(s), Err(DomainError::EmptyReviewComment));
        }
    }

    #[test]
    fn too_long_comment_is_a_domain_error() {
        assert_eq!(
            ReviewComment::new("あ".repeat(ReviewComment::MAX_CHARS + 1)),
            Err(DomainError::ReviewCommentTooLong {
                max: ReviewComment::MAX_CHARS
            })
        );
    }
}
