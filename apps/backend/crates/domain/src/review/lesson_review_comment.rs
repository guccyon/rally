use crate::DomainError;

/// レッスンの評価に添える一言。空（空白だけ）でなく、[`LessonReviewComment::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonReviewComment(String);

impl LessonReviewComment {
    /// 一言の上限（文字数。student-app の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 200;

    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyLessonReviewComment);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::LessonReviewCommentTooLong {
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
            LessonReviewComment::new("フォームの癖がよく分かった")
                .unwrap()
                .as_str(),
            "フォームの癖がよく分かった"
        );
        assert!(LessonReviewComment::new("あ".repeat(LessonReviewComment::MAX_CHARS)).is_ok());
    }

    #[test]
    fn empty_comment_is_a_domain_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(
                LessonReviewComment::new(s),
                Err(DomainError::EmptyLessonReviewComment)
            );
        }
    }

    #[test]
    fn too_long_comment_is_a_domain_error() {
        assert_eq!(
            LessonReviewComment::new("あ".repeat(LessonReviewComment::MAX_CHARS + 1)),
            Err(DomainError::LessonReviewCommentTooLong {
                max: LessonReviewComment::MAX_CHARS
            })
        );
    }
}
