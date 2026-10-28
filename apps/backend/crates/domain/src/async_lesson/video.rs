use std::time::SystemTime;

use super::{AiFeedback, ReviewComment, VideoId, VideoReview, VideoReviewId, VideoStatus};
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
/// コーチの添削の前に AI の一次フィードバック（[`AiFeedback`]）を 1 つ持てる（[`Video::record_ai_feedback`]）。
/// 一次フィードバックは添削とは別の項目で、状態（[`VideoStatus`]）にも添削の着手条件にも関わらず、添削の返却で
/// 変わらない。
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
    ai_feedback: Option<AiFeedback>,
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
            ai_feedback: None,
        }
    }

    /// 保存先から読んだ動画を組み立てる（保存先の実装用）。`review` は返却した添削の ID・コメント・返却した時刻
    /// （返却するまでは `None`）。返却が送付より前なら [`DomainError::ReviewReturnedBeforeSubmission`]。
    pub fn restore(
        id: VideoId,
        student_id: AccountId,
        coach_id: AccountId,
        sport: Sport,
        submitted_at: SystemTime,
        review: Option<(VideoReviewId, ReviewComment, SystemTime)>,
    ) -> Result<Self, DomainError> {
        if review
            .as_ref()
            .is_some_and(|(_, _, returned_at)| *returned_at < submitted_at)
        {
            return Err(DomainError::ReviewReturnedBeforeSubmission);
        }
        Ok(Self {
            id,
            student_id,
            coach_id,
            sport,
            submitted_at,
            review: review.map(|(review_id, comment, returned_at)| {
                VideoReview::new(review_id, id, comment, returned_at)
            }),
            ai_feedback: None,
        })
    }

    /// AI の一次フィードバック `feedback` を動画に付ける（送付の直後に付ける。保存先から読んだ動画を組み立てるときにも
    /// 使う）。次の不変条件を破れば [`DomainError`] を返し、動画を変えない。
    ///
    /// - 一次フィードバックは動画 1 本に 1 つ（[`DomainError::AiFeedbackAlreadyRecorded`]）
    /// - 作ったのは送付の後（同じ時刻は許す。[`DomainError::AiFeedbackBeforeSubmission`]）
    ///
    /// 添削の有無は問わない（一次フィードバックは添削とは別の項目）。
    pub fn record_ai_feedback(&mut self, feedback: AiFeedback) -> Result<(), DomainError> {
        if self.ai_feedback.is_some() {
            return Err(DomainError::AiFeedbackAlreadyRecorded);
        }
        if feedback.created_at() < self.submitted_at {
            return Err(DomainError::AiFeedbackBeforeSubmission);
        }
        self.ai_feedback = Some(feedback);
        Ok(())
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

    /// AI の一次フィードバック。まだない（作る前か、作るのに失敗した）ときは `None`。
    pub const fn ai_feedback(&self) -> Option<&AiFeedback> {
        self.ai_feedback.as_ref()
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

    fn ai_feedback(hour: u64) -> AiFeedback {
        AiFeedback::new("ラケットの面が上を向いています", at(hour)).unwrap()
    }

    #[test]
    fn a_submitted_video_has_no_ai_feedback_until_one_is_recorded() {
        let mut video = submitted();
        assert_eq!(video.ai_feedback(), None);
        video.record_ai_feedback(ai_feedback(0)).unwrap();
        assert_eq!(video.ai_feedback(), Some(&ai_feedback(0)));
        assert_eq!(video.status(), VideoStatus::Submitted);
    }

    #[test]
    fn ai_feedback_is_recorded_once_and_not_before_submission() {
        let mut video = submitted();
        video.record_ai_feedback(ai_feedback(1)).unwrap();
        let before = video.clone();
        assert_eq!(
            video.record_ai_feedback(ai_feedback(2)),
            Err(DomainError::AiFeedbackAlreadyRecorded)
        );
        assert_eq!(video, before);

        let mut video = Video::submit(VIDEO, STUDENT, COACH, Sport::Tennis, at(5));
        assert_eq!(
            video.record_ai_feedback(ai_feedback(4)),
            Err(DomainError::AiFeedbackBeforeSubmission)
        );
        assert_eq!(video.ai_feedback(), None);
    }

    #[test]
    fn returning_a_review_keeps_the_ai_feedback() {
        let mut video = submitted();
        video.record_ai_feedback(ai_feedback(1)).unwrap();
        video
            .return_review(REVIEW, COACH, comment(), true, at(30))
            .unwrap();
        assert_eq!(video.ai_feedback(), Some(&ai_feedback(1)));
        assert_eq!(video.review().unwrap().comment(), &comment());

        // 返却した後の動画にも付けられる（添削とは別の項目）
        let mut reviewed = reviewed();
        reviewed.record_ai_feedback(ai_feedback(31)).unwrap();
        assert_eq!(reviewed.ai_feedback(), Some(&ai_feedback(31)));
    }

    #[test]
    fn restore_round_trips_a_reviewed_video_and_rejects_returning_before_submission() {
        let mut video = submitted();
        video
            .return_review(REVIEW, COACH, comment(), true, at(5))
            .unwrap();

        let restored = Video::restore(
            VIDEO,
            STUDENT,
            COACH,
            Sport::Tennis,
            NOW,
            Some((REVIEW, comment(), at(5))),
        )
        .unwrap();
        assert_eq!(restored, video);

        assert_eq!(
            Video::restore(
                VIDEO,
                STUDENT,
                COACH,
                Sport::Tennis,
                at(5),
                Some((REVIEW, comment(), at(4))),
            ),
            Err(DomainError::ReviewReturnedBeforeSubmission)
        );
    }

    #[test]
    fn restore_accepts_returning_at_or_after_submission() {
        let restore = |returned_at| {
            Video::restore(
                VIDEO,
                STUDENT,
                COACH,
                Sport::Tennis,
                at(5),
                Some((REVIEW, comment(), returned_at)),
            )
        };
        // 送付の 1 時間前は断る
        assert_eq!(
            restore(at(4)),
            Err(DomainError::ReviewReturnedBeforeSubmission)
        );
        // 送付と同じ時刻と、その 1 時間後は読める
        for returned_at in [at(5), at(6)] {
            let video = restore(returned_at).unwrap();
            assert_eq!(video.status(), VideoStatus::Reviewed);
            assert_eq!(
                video.review().map(VideoReview::returned_at),
                Some(returned_at)
            );
        }
    }
}
