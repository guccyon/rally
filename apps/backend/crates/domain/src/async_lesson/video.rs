use std::time::SystemTime;

use super::{ReviewComment, VideoId, VideoReview, VideoReviewId, VideoStatus};
use crate::{
    DomainError,
    identity::{AccountId, Sport},
    lesson::{LessonCompletion, LessonId, LessonSource},
};

/// 非同期レッスン（glossary の async_lesson）で生徒がコーチに送る動画。
///
/// 送付（[`Video::submit`]）した直後は [`VideoStatus::Submitted`] で、動画のコーチが添削を返す
/// （[`Video::return_review`]）と [`VideoReview`] を持つ [`VideoStatus::Reviewed`] になる。動画を扱えるのは
/// 送った生徒と送り先のコーチ（当事者）だけ。
///
/// 送付の回数は数えない（月額は送り放題）。添削に着手してよいか（ゴルフはワンショット決済が確定した動画だけ。
/// 不変条件 2）は [`crate::entitlement::ContractType::may_start_review`] が決め、返却はその答えに従う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    id: VideoId,
    student_id: AccountId,
    coach_id: AccountId,
    sport: Sport,
    submitted_at: SystemTime,
    review: Option<VideoReview>,
}

impl Video {
    /// 生徒 `student_id` がコーチ `coach_id` に `sport` の動画を `now` に送る。
    pub const fn submit(
        id: VideoId,
        student_id: AccountId,
        coach_id: AccountId,
        sport: Sport,
        now: SystemTime,
    ) -> Self {
        Self {
            id,
            student_id,
            coach_id,
            sport,
            submitted_at: now,
            review: None,
        }
    }

    /// `account` が動画の当事者（送った生徒か送り先のコーチ）でなければ [`DomainError::NotAVideoParty`]。
    pub fn ensure_visible_to(&self, account: AccountId) -> Result<(), DomainError> {
        if account == self.student_id || account == self.coach_id {
            Ok(())
        } else {
            Err(DomainError::NotAVideoParty)
        }
    }

    /// コーチ `coach` が `now` に添削 `comment` を返す（添削の ID は `review_id`）。
    ///
    /// `may_start_review` は添削に着手してよいかの答え（[`crate::entitlement::ContractType::may_start_review`]）。
    /// 次の不変条件を破れば [`DomainError`] を返し、動画を変えない。
    ///
    /// - 返せるのは送り先のコーチだけ（[`DomainError::NotTheVideoCoach`]）
    /// - 返却は 1 回（[`DomainError::VideoReviewAlreadyReturned`]）
    /// - 着手できない動画は返せない（[`DomainError::VideoReviewNotAllowed`]。ゴルフで決済が確定していない）
    /// - 返却は送付の後（同じ時刻は許す。[`DomainError::ReviewReturnedBeforeSubmission`]）
    pub fn return_review(
        &mut self,
        review_id: VideoReviewId,
        coach: AccountId,
        comment: ReviewComment,
        may_start_review: bool,
        now: SystemTime,
    ) -> Result<(), DomainError> {
        if coach != self.coach_id {
            return Err(DomainError::NotTheVideoCoach);
        }
        if self.review.is_some() {
            return Err(DomainError::VideoReviewAlreadyReturned);
        }
        if !may_start_review {
            return Err(DomainError::VideoReviewNotAllowed { sport: self.sport });
        }
        if now < self.submitted_at {
            return Err(DomainError::ReviewReturnedBeforeSubmission);
        }
        self.review = Some(VideoReview::new(review_id, self.id, comment, now));
        Ok(())
    }

    /// 当事者 `account` が `now` に、この動画の非同期レッスンの完了を記録する（完了記録の ID は `lesson_id`）。
    ///
    /// `history` はその生徒の既存の完了記録（同期・非同期の両方。`is_first_lesson` の判定と重複の検出に使う。
    /// [`LessonCompletion`]）。次の不変条件を破れば [`DomainError`] を返す。動画は変えない。
    ///
    /// - 記録できるのは動画の当事者（送った生徒・送り先のコーチ）だけ（[`DomainError::NotAVideoParty`]）
    /// - 添削を返した動画だけ（[`DomainError::VideoReviewNotReturned`]）
    /// - 記録は返却の後（同じ時刻は許す。[`DomainError::CompletionBeforeLessonEnded`]）
    /// - 同じ動画の完了は 1 回（[`DomainError::LessonAlreadyCompleted`]）
    pub fn complete(
        &self,
        lesson_id: LessonId,
        account: AccountId,
        history: &[LessonCompletion],
        now: SystemTime,
    ) -> Result<LessonCompletion, DomainError> {
        self.ensure_visible_to(account)?;
        let review = self
            .review
            .as_ref()
            .ok_or(DomainError::VideoReviewNotReturned)?;
        if now < review.returned_at() {
            return Err(DomainError::CompletionBeforeLessonEnded);
        }
        LessonCompletion::record(
            lesson_id,
            self.student_id,
            self.coach_id,
            LessonSource::Async(self.id),
            history,
            now,
        )
    }

    pub const fn id(&self) -> VideoId {
        self.id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn sport(&self) -> Sport {
        self.sport
    }

    pub const fn submitted_at(&self) -> SystemTime {
        self.submitted_at
    }

    pub const fn status(&self) -> VideoStatus {
        match self.review {
            None => VideoStatus::Submitted,
            Some(_) => VideoStatus::Reviewed,
        }
    }

    /// 返却した添削。返却するまでは `None`。
    pub const fn review(&self) -> Option<&VideoReview> {
        self.review.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(3);
    const VIDEO: VideoId = VideoId::from_u128(10);
    const REVIEW: VideoReviewId = VideoReviewId::from_u128(20);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;

    fn at(hour: u64) -> SystemTime {
        NOW + Duration::from_secs(hour * 3600)
    }

    fn submitted() -> Video {
        Video::submit(VIDEO, STUDENT, COACH, Sport::Tennis, NOW)
    }

    fn comment() -> ReviewComment {
        ReviewComment::new("テイクバックが早い").unwrap()
    }

    #[test]
    fn a_submitted_video_awaits_its_review() {
        let video = submitted();
        assert_eq!(video.id(), VIDEO);
        assert_eq!(
            (video.student_id(), video.coach_id(), video.sport()),
            (STUDENT, COACH, Sport::Tennis)
        );
        assert_eq!(video.submitted_at(), NOW);
        assert_eq!(video.status(), VideoStatus::Submitted);
        assert_eq!(video.review(), None);
    }

    #[test]
    fn only_the_parties_can_see_a_video() {
        let video = submitted();
        assert_eq!(video.ensure_visible_to(STUDENT), Ok(()));
        assert_eq!(video.ensure_visible_to(COACH), Ok(()));
        assert_eq!(
            video.ensure_visible_to(OTHER_COACH),
            Err(DomainError::NotAVideoParty)
        );
    }

    #[test]
    fn the_coach_returns_a_review() {
        let mut video = submitted();
        video
            .return_review(REVIEW, COACH, comment(), true, at(30))
            .unwrap();
        assert_eq!(video.status(), VideoStatus::Reviewed);
        let review = video.review().unwrap();
        assert_eq!(review.id(), REVIEW);
        assert_eq!(review.video_id(), VIDEO);
        assert_eq!(review.comment(), &comment());
        assert_eq!(review.returned_at(), at(30));
    }

    #[test]
    fn a_review_can_be_returned_at_the_moment_of_submission() {
        let mut video = submitted();
        video
            .return_review(REVIEW, COACH, comment(), true, NOW)
            .unwrap();
        assert_eq!(video.status(), VideoStatus::Reviewed);
    }

    #[test]
    fn return_failures_change_nothing() {
        let mut reviewed = submitted();
        reviewed
            .return_review(REVIEW, COACH, comment(), true, at(1))
            .unwrap();

        for (mut video, coach, may_start_review, now, expected) in [
            (
                submitted(),
                OTHER_COACH,
                true,
                at(1),
                DomainError::NotTheVideoCoach,
            ),
            (
                submitted(),
                STUDENT,
                true,
                at(1),
                DomainError::NotTheVideoCoach,
            ),
            (
                reviewed.clone(),
                COACH,
                true,
                at(2),
                DomainError::VideoReviewAlreadyReturned,
            ),
            (
                submitted(),
                COACH,
                false,
                at(1),
                DomainError::VideoReviewNotAllowed {
                    sport: Sport::Tennis,
                },
            ),
            (
                Video::submit(VIDEO, STUDENT, COACH, Sport::Tennis, at(5)),
                COACH,
                true,
                at(4),
                DomainError::ReviewReturnedBeforeSubmission,
            ),
        ] {
            let before = video.clone();
            assert_eq!(
                video.return_review(
                    VideoReviewId::from_u128(21),
                    coach,
                    comment(),
                    may_start_review,
                    now
                ),
                Err(expected)
            );
            assert_eq!(video, before);
        }
    }

    const LESSON: LessonId = LessonId::from_u128(70);

    fn reviewed() -> Video {
        let mut video = submitted();
        video
            .return_review(REVIEW, COACH, comment(), true, at(30))
            .unwrap();
        video
    }

    #[test]
    fn the_parties_complete_a_reviewed_video() {
        let video = reviewed();
        for account in [STUDENT, COACH] {
            let completion = video.complete(LESSON, account, &[], at(31)).unwrap();
            assert_eq!(completion.id(), LESSON);
            assert_eq!(
                (completion.student_id(), completion.coach_id()),
                (STUDENT, COACH)
            );
            assert_eq!(completion.source(), LessonSource::Async(VIDEO));
            assert_eq!(completion.lesson_type(), crate::lesson::LessonType::Async);
            assert_eq!(completion.completed_at(), at(31));
            assert!(completion.is_first_lesson());
        }
        // 返却と同じ時刻にも記録できる
        video.complete(LESSON, STUDENT, &[], at(30)).unwrap();
    }

    #[test]
    fn completion_failures() {
        let done = reviewed().complete(LESSON, STUDENT, &[], at(31)).unwrap();
        for (video, account, history, now, expected) in [
            (
                reviewed(),
                OTHER_COACH,
                vec![],
                at(31),
                DomainError::NotAVideoParty,
            ),
            (
                submitted(),
                STUDENT,
                vec![],
                at(31),
                DomainError::VideoReviewNotReturned,
            ),
            (
                reviewed(),
                STUDENT,
                vec![],
                at(29),
                DomainError::CompletionBeforeLessonEnded,
            ),
            (
                reviewed(),
                COACH,
                vec![done],
                at(40),
                DomainError::LessonAlreadyCompleted,
            ),
        ] {
            assert_eq!(
                video.complete(LessonId::from_u128(71), account, &history, now),
                Err(expected)
            );
        }
    }
}
