use std::time::SystemTime;

use super::LessonReviewReplyComment;

/// レッスンの評価へのコーチの返信（glossary の lesson_review_reply）。評価 1 件に 1 つで、付けるのは評価されたコーチ。
///
/// 付け直すと文と時刻を丸ごと置き換える（[`super::LessonReview::write_reply`]）。削除はしない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonReviewReply {
    comment: LessonReviewReplyComment,
    replied_at: SystemTime,
}

impl LessonReviewReply {
    pub const fn new(comment: LessonReviewReplyComment, replied_at: SystemTime) -> Self {
        Self {
            comment,
            replied_at,
        }
    }

    pub const fn comment(&self) -> &LessonReviewReplyComment {
        &self.comment
    }

    /// 返信を付けた（付け直したなら最後に付け直した）時刻。
    pub const fn replied_at(&self) -> SystemTime {
        self.replied_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_comment_and_time() {
        let reply = LessonReviewReply::new(
            LessonReviewReplyComment::new("ありがとうございます").unwrap(),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(reply.comment().as_str(), "ありがとうございます");
        assert_eq!(reply.replied_at(), SystemTime::UNIX_EPOCH);
    }
}
