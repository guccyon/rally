use std::time::SystemTime;

use crate::DomainError;

/// AI の一次フィードバック（glossary の ai_feedback）。生徒が送った動画に、コーチの添削（[`super::VideoReview`]）の
/// 前に AI が返す短い文と、それを作った時刻。コーチの添削とは別の項目で、添削の返却で上書きされない。
///
/// 文は空（空白だけ）でない。動画に付けるのは [`super::Video::record_ai_feedback`]（付ける条件はそこで確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiFeedback {
    comment: String,
    created_at: SystemTime,
}

impl AiFeedback {
    /// `created_at` に作った一次フィードバック `comment`。空（空白だけ）なら [`DomainError::EmptyAiFeedback`]。
    pub fn new(comment: impl Into<String>, created_at: SystemTime) -> Result<Self, DomainError> {
        let comment = comment.into();
        if comment.trim().is_empty() {
            return Err(DomainError::EmptyAiFeedback);
        }
        Ok(Self {
            comment,
            created_at,
        })
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub const fn created_at(&self) -> SystemTime {
        self.created_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_comment_and_when_it_was_created() {
        let feedback = AiFeedback::new("フォームを確認しました", SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(feedback.comment(), "フォームを確認しました");
        assert_eq!(feedback.created_at(), SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn empty_comment_is_a_domain_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(
                AiFeedback::new(s, SystemTime::UNIX_EPOCH),
                Err(DomainError::EmptyAiFeedback)
            );
        }
    }
}
