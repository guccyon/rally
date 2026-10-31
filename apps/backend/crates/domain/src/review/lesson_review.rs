use std::time::SystemTime;

use super::{LessonReviewComment, LessonReviewId, Rating, ReviewError};
use crate::{
    identity::AccountId,
    lesson::{LessonCompletion, LessonId},
};

/// レッスンの評価（glossary の lesson_review）。完了したレッスン 1 件につき 1 つで、付けるのはそのレッスンの生徒。
///
/// 評価されるコーチは完了したレッスンのコーチで、評価する側は選べない。編集・削除は Gen1 の範囲外（作ったら変えない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonReview {
    id: LessonReviewId,
    lesson_id: LessonId,
    student_id: AccountId,
    coach_id: AccountId,
    rating: Rating,
    comment: LessonReviewComment,
    submitted_at: SystemTime,
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
        })
    }

    /// 保存先から読んだ評価を組み立てる（保存先の実装用）。
    pub const fn restore(
        id: LessonReviewId,
        lesson_id: LessonId,
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
        }
    }

    pub const fn id(&self) -> LessonReviewId {
        self.id
    }

    pub const fn lesson_id(&self) -> LessonId {
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
            .complete(LessonId::from_u128(id), STUDENT, &[], at(2))
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

    #[test]
    fn restore_keeps_every_field() {
        let review = submit(STUDENT, &lesson(10), None).unwrap();
        let restored = LessonReview::restore(
            review.id(),
            review.lesson_id(),
            review.student_id(),
            review.coach_id(),
            review.rating(),
            review.comment().clone(),
            review.submitted_at(),
        );
        assert_eq!(restored, review);
    }
}
