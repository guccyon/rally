use std::time::SystemTime;

use super::{
    LessonReviewComment, LessonReviewId, LessonReviewReply, LessonReviewReplyComment,
    LowRatedReviewCriteria, LowRatedReviewHandling, Rating, ReviewError,
};
use crate::{
    identity::AccountId,
    lesson::{LessonCompletion, LessonCompletionId},
};

/// レッスンの評価（glossary の lesson_review）。完了したレッスン 1 件につき 1 つで、付けるのはそのレッスンの生徒。
///
/// 評価されるコーチは完了したレッスンのコーチで、評価する側は選べない。編集・削除は Gen1 の範囲外（作ったら点と一言は
/// 変えない）。評価されたコーチは返信（[`LessonReviewReply`]）を 1 つだけ付けられ、付け直すと上書きする（[`Self::write_reply`]）。
/// 低い評価には本部が「対応済み」の印（[`LowRatedReviewHandling`]）を 1 つだけ付けられ、2 回目は何も変えない
/// （[`Self::mark_handled`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonReview {
    id: LessonReviewId,
    lesson_id: LessonCompletionId,
    student_id: AccountId,
    coach_id: AccountId,
    rating: Rating,
    comment: LessonReviewComment,
    submitted_at: SystemTime,
    reply: Option<LessonReviewReply>,
    handling: Option<LowRatedReviewHandling>,
}

impl LessonReview {
    /// アカウント `account_id` が完了したレッスン `lesson` に評価を付ける。
    ///
    /// `existing` はそのレッスンの既存の評価。付けられるのはレッスンの生徒だけ（それ以外は
    /// [`ReviewError::NotTheLessonStudent`]）で、評価がすでにあれば [`ReviewError::LessonAlreadyReviewed`]。
    pub fn submit(
        id: LessonReviewId,
        account_id: AccountId,
        lesson: &LessonCompletion,
        existing: Option<&LessonReview>,
        rating: Rating,
        comment: LessonReviewComment,
        now: SystemTime,
    ) -> Result<Self, ReviewError> {
        if account_id != lesson.student_id() {
            return Err(ReviewError::NotTheLessonStudent);
        }
        if existing.is_some_and(|review| review.lesson_id == lesson.id()) {
            return Err(ReviewError::LessonAlreadyReviewed);
        }
        Ok(Self {
            id,
            lesson_id: lesson.id(),
            student_id: lesson.student_id(),
            coach_id: lesson.coach_id(),
            rating,
            comment,
            submitted_at: now,
            reply: None,
            handling: None,
        })
    }

    /// 保存先から読んだ評価を組み立てる（保存先の実装用）。返信は [`Self::with_reply`]、対応済みの印は
    /// [`Self::with_handling`] で添える（既定はどちらもなし）。
    pub const fn restore(
        id: LessonReviewId,
        lesson_id: LessonCompletionId,
        student_id: AccountId,
        coach_id: AccountId,
        rating: Rating,
        comment: LessonReviewComment,
        submitted_at: SystemTime,
    ) -> Self {
        Self {
            id,
            lesson_id,
            student_id,
            coach_id,
            rating,
            comment,
            submitted_at,
            reply: None,
            handling: None,
        }
    }

    /// 保存先から読んだ返信 `reply` を添える（保存先の実装用。[`Self::restore`] の続き）。
    pub fn with_reply(self, reply: Option<LessonReviewReply>) -> Self {
        Self { reply, ..self }
    }

    /// 保存先から読んだ対応済みの印 `handling` を添える（保存先の実装用。[`Self::restore`] の続き）。
    pub fn with_handling(self, handling: Option<LowRatedReviewHandling>) -> Self {
        Self { handling, ..self }
    }

    /// アカウント `account_id` がこの評価に返信 `comment` を `now` に付ける。返信がすでにあれば文と時刻を置き換える。
    ///
    /// 付けられるのは評価されたコーチだけ（それ以外は [`ReviewError::NotTheReviewedCoach`] で、何も変えない）。
    pub fn write_reply(
        &mut self,
        account_id: AccountId,
        comment: LessonReviewReplyComment,
        now: SystemTime,
    ) -> Result<(), ReviewError> {
        if account_id != self.coach_id {
            return Err(ReviewError::NotTheReviewedCoach);
        }
        self.reply = Some(LessonReviewReply::new(comment, now));
        Ok(())
    }

    /// 本部のアカウント `handled_by` がこの評価に「対応済み」の印を `now` に付ける。
    ///
    /// 付けられるのは低い点（[`LowRatedReviewCriteria::is_low_rating`]。期間は問わない）の評価だけで、それ以外は
    /// [`ReviewError::NotLowRated`]（何も変えない）。すでに印があれば何も変えずに成功する（最初のアカウントと時刻を残す）。
    /// 本部の役割かは境界が確かめる。
    pub fn mark_handled(
        &mut self,
        handled_by: AccountId,
        now: SystemTime,
    ) -> Result<(), ReviewError> {
        if !LowRatedReviewCriteria::is_low_rating(self.rating) {
            return Err(ReviewError::NotLowRated);
        }
        if self.handling.is_none() {
            self.handling = Some(LowRatedReviewHandling::new(handled_by, now));
        }
        Ok(())
    }

    pub const fn id(&self) -> LessonReviewId {
        self.id
    }

    pub const fn lesson_id(&self) -> LessonCompletionId {
        self.lesson_id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn rating(&self) -> Rating {
        self.rating
    }

    pub const fn comment(&self) -> &LessonReviewComment {
        &self.comment
    }

    pub const fn submitted_at(&self) -> SystemTime {
        self.submitted_at
    }

    /// 評価されたコーチの返信。まだなければ `None`。
    pub const fn reply(&self) -> Option<&LessonReviewReply> {
        self.reply.as_ref()
    }

    /// 本部の「対応済み」の印。まだなければ `None`。
    pub const fn handling(&self) -> Option<&LowRatedReviewHandling> {
        self.handling.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
        catalog::Sport,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(3);

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    /// 生徒 1 がコーチ 3 から受けた非同期レッスンの完了記録
    fn lesson(id: u128) -> LessonCompletion {
        let mut video = Video::submit(VideoId::from_u128(id), STUDENT, COACH, Sport::Tennis, at(0));
        video
            .return_review(
                VideoReviewId::from_u128(id),
                COACH,
                ReviewComment::new("よい").unwrap(),
                true,
                at(1),
            )
            .unwrap();
        video
            .complete(LessonCompletionId::from_u128(id), STUDENT, &[], at(2))
            .unwrap()
    }

    fn submit(
        account_id: AccountId,
        lesson: &LessonCompletion,
        existing: Option<&LessonReview>,
    ) -> Result<LessonReview, ReviewError> {
        LessonReview::submit(
            LessonReviewId::from_u128(9),
            account_id,
            lesson,
            existing,
            Rating::new(4).unwrap(),
            LessonReviewComment::new("分かりやすかった").unwrap(),
            at(3),
        )
    }

    #[test]
    fn the_student_of_the_lesson_reviews_its_coach() {
        let lesson = lesson(10);
        let review = submit(STUDENT, &lesson, None).unwrap();
        assert_eq!(review.id(), LessonReviewId::from_u128(9));
        assert_eq!(review.lesson_id(), lesson.id());
        assert_eq!((review.student_id(), review.coach_id()), (STUDENT, COACH));
        assert_eq!(review.rating().value(), 4);
        assert_eq!(review.comment().as_str(), "分かりやすかった");
        assert_eq!(review.submitted_at(), at(3));
        assert_eq!(review.reply(), None);
        assert_eq!(review.handling(), None);
    }

    #[test]
    fn only_the_student_of_the_lesson_can_review_it() {
        let lesson = lesson(10);
        for account in [OTHER_STUDENT, COACH] {
            assert_eq!(
                submit(account, &lesson, None),
                Err(ReviewError::NotTheLessonStudent)
            );
        }
    }

    #[test]
    fn a_lesson_is_reviewed_only_once() {
        let lesson = lesson(10);
        let first = submit(STUDENT, &lesson, None).unwrap();
        assert_eq!(
            submit(STUDENT, &lesson, Some(&first)),
            Err(ReviewError::LessonAlreadyReviewed)
        );
        // 当事者でなければ、評価があることも知らせない
        assert_eq!(
            submit(OTHER_STUDENT, &lesson, Some(&first)),
            Err(ReviewError::NotTheLessonStudent)
        );
        // 別のレッスンの評価は数えない
        assert!(
            submit(
                STUDENT,
                &lesson,
                Some(&submit(STUDENT, &self::lesson(11), None).unwrap())
            )
            .is_ok()
        );
    }

    fn reply(text: &str) -> LessonReviewReplyComment {
        LessonReviewReplyComment::new(text).unwrap()
    }

    #[test]
    fn the_reviewed_coach_replies_and_a_second_reply_overwrites_the_first() {
        let mut review = submit(STUDENT, &lesson(10), None).unwrap();

        review
            .write_reply(COACH, reply("ありがとう"), at(4))
            .unwrap();
        assert_eq!(
            review.reply(),
            Some(&LessonReviewReply::new(reply("ありがとう"), at(4)))
        );

        review
            .write_reply(COACH, reply("次回はサーブも見ます"), at(5))
            .unwrap();
        assert_eq!(
            review.reply(),
            Some(&LessonReviewReply::new(
                reply("次回はサーブも見ます"),
                at(5)
            )),
            "文と時刻を置き換える"
        );
        // 点と一言は変えない
        assert_eq!(review.rating().value(), 4);
        assert_eq!(review.comment().as_str(), "分かりやすかった");
    }

    #[test]
    fn only_the_reviewed_coach_can_reply() {
        let mut review = submit(STUDENT, &lesson(10), None).unwrap();
        review
            .write_reply(COACH, reply("ありがとう"), at(4))
            .unwrap();
        let before = review.clone();
        for account in [STUDENT, OTHER_STUDENT, AccountId::from_u128(4)] {
            assert_eq!(
                review.write_reply(account, reply("横取り"), at(5)),
                Err(ReviewError::NotTheReviewedCoach),
                "{account:?}"
            );
        }
        assert_eq!(review, before, "拒んだら何も変えない");
    }

    const HEADQUARTERS: AccountId = AccountId::from_u128(5);
    const OTHER_HEADQUARTERS: AccountId = AccountId::from_u128(6);

    fn rated(rating: i64) -> LessonReview {
        LessonReview::submit(
            LessonReviewId::from_u128(9),
            STUDENT,
            &lesson(10),
            None,
            Rating::new(rating).unwrap(),
            LessonReviewComment::new("いまひとつ").unwrap(),
            at(3),
        )
        .unwrap()
    }

    #[test]
    fn headquarters_marks_a_low_review_handled_once_and_a_second_mark_changes_nothing() {
        for rating in [1, 2] {
            let mut review = rated(rating);
            review.mark_handled(HEADQUARTERS, at(4)).unwrap();
            assert_eq!(
                review.handling(),
                Some(&LowRatedReviewHandling::new(HEADQUARTERS, at(4))),
                "★{rating}"
            );

            let first = review.clone();
            review.mark_handled(OTHER_HEADQUARTERS, at(5)).unwrap();
            assert_eq!(review, first, "最初のアカウントと時刻を残す");
        }
    }

    #[test]
    fn only_a_low_review_can_be_marked_handled() {
        for rating in [3, 4, 5] {
            let mut review = rated(rating);
            let before = review.clone();
            assert_eq!(
                review.mark_handled(HEADQUARTERS, at(4)),
                Err(ReviewError::NotLowRated),
                "★{rating}"
            );
            assert_eq!(review, before, "拒んだら何も変えない");
        }
    }

    #[test]
    fn marking_handled_keeps_the_rating_comment_and_reply() {
        let mut review = rated(1);
        review
            .write_reply(COACH, reply("ご意見ありがとうございます"), at(4))
            .unwrap();
        review.mark_handled(HEADQUARTERS, at(5)).unwrap();
        assert_eq!(review.rating().value(), 1);
        assert_eq!(review.comment().as_str(), "いまひとつ");
        assert_eq!(
            review.reply(),
            Some(&LessonReviewReply::new(
                reply("ご意見ありがとうございます"),
                at(4)
            ))
        );
    }

    #[test]
    fn restore_keeps_every_field() {
        let mut review = submit(STUDENT, &lesson(10), None).unwrap();
        let restore = |review: &LessonReview| {
            LessonReview::restore(
                review.id(),
                review.lesson_id(),
                review.student_id(),
                review.coach_id(),
                review.rating(),
                review.comment().clone(),
                review.submitted_at(),
            )
            .with_reply(review.reply().cloned())
            .with_handling(review.handling().cloned())
        };
        assert_eq!(restore(&review), review);
        review
            .write_reply(COACH, reply("ありがとう"), at(4))
            .unwrap();
        assert_eq!(restore(&review), review);

        let mut low = rated(2);
        low.mark_handled(HEADQUARTERS, at(4)).unwrap();
        assert_eq!(restore(&low), low);
    }
}
