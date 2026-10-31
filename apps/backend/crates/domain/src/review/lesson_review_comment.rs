use super::ReviewError;

/// レッスンの評価に添える一言。空（空白だけ）でなく、[`LessonReviewComment::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonReviewComment(String);

impl LessonReviewComment {
    /// 一言の上限（文字数。student-app の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 200;

    pub fn new(value: impl Into<String>) -> Result<Self, ReviewError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ReviewError::EmptyLessonReviewComment);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(ReviewError::LessonReviewCommentTooLong {
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
    fn empty_comment_is_a_review_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(
                LessonReviewComment::new(s),
                Err(ReviewError::EmptyLessonReviewComment)
            );
        }
    }

    #[test]
    fn too_long_comment_is_a_review_error() {
        assert_eq!(
            LessonReviewComment::new("あ".repeat(LessonReviewComment::MAX_CHARS + 1)),
            Err(ReviewError::LessonReviewCommentTooLong {
                max: LessonReviewComment::MAX_CHARS
            })
        );
    }
}
