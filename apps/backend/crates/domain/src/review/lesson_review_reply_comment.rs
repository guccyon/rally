use super::ReviewError;

/// レッスンの評価へのコーチの返信の文。空（空白だけ）でなく、[`LessonReviewReplyComment::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonReviewReplyComment(String);

impl LessonReviewReplyComment {
    /// 返信の文の上限（文字数。admin-web の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 200;

    pub fn new(value: impl Into<String>) -> Result<Self, ReviewError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ReviewError::EmptyLessonReviewReply);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(ReviewError::LessonReviewReplyTooLong {
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
    fn accepts_non_empty_replies_up_to_the_limit() {
        assert_eq!(
            LessonReviewReplyComment::new("ありがとうございます。次回も一緒に頑張りましょう")
                .unwrap()
                .as_str(),
            "ありがとうございます。次回も一緒に頑張りましょう"
        );
        assert!(
            LessonReviewReplyComment::new("あ".repeat(LessonReviewReplyComment::MAX_CHARS)).is_ok()
        );
    }

    #[test]
    fn empty_reply_is_a_review_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(
                LessonReviewReplyComment::new(s),
                Err(ReviewError::EmptyLessonReviewReply)
            );
        }
    }

    #[test]
    fn too_long_reply_is_a_review_error() {
        assert_eq!(
            LessonReviewReplyComment::new("あ".repeat(LessonReviewReplyComment::MAX_CHARS + 1)),
            Err(ReviewError::LessonReviewReplyTooLong {
                max: LessonReviewReplyComment::MAX_CHARS
            })
        );
    }
}
