use domain::{
    DomainError,
    async_lesson::{ReviewComment, Video, VideoId},
    identity::AccountId,
};

use super::{ReviewGate, UpdateVideoError, VideoRepository};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// 添削の返却の入力。`coach_id` は認証したコーチのアカウント（役割の確認は境界が、動画の送り先かは domain が
/// 確かめる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnVideoReviewCommand {
    pub coach_id: AccountId,
    pub video_id: VideoId,
    pub comment: ReviewComment,
}

#[derive(Debug)]
pub enum ReturnVideoReviewError {
    /// 動画がない
    VideoNotFound,
    /// 返却がドメインの不変条件を満たさない（送り先のコーチでない、返却済み、着手できない動画、
    /// 返却が送付より前）
    Invalid(DomainError),
    /// 保存先・問い合わせ先の障害
    Unavailable(Unavailable),
}

/// 添削の返却のユースケース（admin-web の `returnVideoReview`。コーチだけ）。
///
/// 動画を読み、添削に着手してよいかを問い合わせ（[`ReviewGate`]。ゴルフはワンショット決済が確定した動画だけ。
/// 不変条件 2）、domain の [`Video::return_review`] で添削を記録して保存する。保存したら行動イベント
/// `video_review.returned` を 1 回発火する。返却のリードタイムをセマンティック層で計算できるよう、送付の時刻
/// （`submitted_at`）も載せる。拒否・失敗では発火しない。
pub struct ReturnVideoReview<V, G, A, C> {
    videos: V,
    gate: G,
    analytics: A,
    clock: C,
}

impl<V, G, A, C> ReturnVideoReview<V, G, A, C>
where
    V: VideoRepository,
    G: ReviewGate,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(videos: V, gate: G, analytics: A, clock: C) -> Self {
        Self {
            videos,
            gate,
            analytics,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: ReturnVideoReviewCommand,
    ) -> Result<Video, ReturnVideoReviewError> {
        let now = self.clock.now();
        let video = self
            .videos
            .find(command.video_id)
            .await
            .map_err(ReturnVideoReviewError::Unavailable)?
            .ok_or(ReturnVideoReviewError::VideoNotFound)?;
        let may_start_review = self
            .gate
            .may_start_review(video.id(), video.sport())
            .await
            .map_err(ReturnVideoReviewError::Unavailable)?;

        let review_id = self.videos.next_review_id();
        let video = self
            .videos
            .update(command.video_id, |video| {
                video.return_review(
                    review_id,
                    command.coach_id,
                    command.comment,
                    may_start_review,
                    now,
                )
            })
            .await
            .map_err(|error| match error {
                UpdateVideoError::NotFound => ReturnVideoReviewError::VideoNotFound,
                UpdateVideoError::Rejected(error) => ReturnVideoReviewError::Invalid(error),
                UpdateVideoError::Unavailable(error) => ReturnVideoReviewError::Unavailable(error),
            })?;

        self.analytics
            .emit(AnalyticsEvent::VideoReviewReturned {
                student_id: video.student_id(),
                coach_id: video.coach_id(),
                video_id: video.id(),
                review_id,
                submitted_at: video.submitted_at(),
                occurred_at: now,
            })
            .await;

        Ok(video)
    }

    pub fn videos(&self) -> &V {
        &self.videos
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{VideoContentType, VideoStatus},
        identity::Sport,
    };

    use super::*;
    use crate::{
        async_lesson::{GetVideoReview, GetVideoReviewQuery, SubmitVideo, SubmitVideoCommand},
        entitlement::EntitlementReviewGate,
        in_memory::{
            FixedClock, FixedVideoStorage, InMemoryVideoRepository, ManualClock,
            RecordingAnalyticsSink,
        },
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const TENNIS_VIDEO: VideoId = VideoId::from_u128(10);
    const GOLF_VIDEO: VideoId = VideoId::from_u128(11);

    fn submitted_at() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn returned_at() -> SystemTime {
        submitted_at() + Duration::from_secs(30 * 3600)
    }

    fn comment() -> ReviewComment {
        ReviewComment::new("テイクバックが早い").unwrap()
    }

    fn command(coach_id: AccountId, video_id: VideoId) -> ReturnVideoReviewCommand {
        ReturnVideoReviewCommand {
            coach_id,
            video_id,
            comment: comment(),
        }
    }

    /// テニスの動画 10 とゴルフの動画 11（どちらも生徒 1 がコーチ 7 に送った）を置く。
    fn use_case() -> ReturnVideoReview<
        InMemoryVideoRepository,
        EntitlementReviewGate,
        RecordingAnalyticsSink,
        FixedClock,
    > {
        ReturnVideoReview::new(
            InMemoryVideoRepository::with_videos([
                Video::submit(TENNIS_VIDEO, STUDENT, COACH, Sport::Tennis, submitted_at()),
                Video::submit(GOLF_VIDEO, STUDENT, COACH, Sport::Golf, submitted_at()),
            ]),
            EntitlementReviewGate,
            RecordingAnalyticsSink::default(),
            FixedClock(returned_at()),
        )
    }

    #[tokio::test]
    async fn submitted_video_is_unreturned_until_the_coach_returns_its_review() {
        let videos = InMemoryVideoRepository::default();
        let analytics = RecordingAnalyticsSink::default();
        let clock = ManualClock::new(submitted_at());
        let submit = SubmitVideo::new(
            videos.clone(),
            FixedVideoStorage,
            analytics.clone(),
            clock.clone(),
        );
        let get = GetVideoReview::new(videos.clone());
        let return_review = ReturnVideoReview::new(
            videos,
            EntitlementReviewGate,
            analytics.clone(),
            clock.clone(),
        );

        let video_id = submit
            .execute(SubmitVideoCommand {
                student_id: STUDENT,
                coach_id: COACH,
                sport: Sport::Tennis,
                content_type: VideoContentType::QuickTime,
            })
            .await
            .unwrap()
            .video
            .id();
        let read = || {
            get.execute(GetVideoReviewQuery {
                account_id: STUDENT,
                video_id,
            })
        };

        // 返却前は未返却
        let before = read().await.unwrap();
        assert_eq!(before.status(), VideoStatus::Submitted);
        assert_eq!(before.review(), None);

        clock.set(returned_at());
        return_review
            .execute(command(COACH, video_id))
            .await
            .unwrap();

        let after = read().await.unwrap();
        assert_eq!(after.status(), VideoStatus::Reviewed);
        let review = after.review().unwrap();
        assert_eq!(review.comment(), &comment());
        assert_eq!(review.returned_at(), returned_at());

        let events = analytics.events();
        assert_eq!(
            events.iter().map(AnalyticsEvent::name).collect::<Vec<_>>(),
            ["video.submitted", "video_review.returned"]
        );
        assert_eq!(
            events[1],
            AnalyticsEvent::VideoReviewReturned {
                student_id: STUDENT,
                coach_id: COACH,
                video_id,
                review_id: review.id(),
                submitted_at: submitted_at(),
                occurred_at: returned_at(),
            },
            "返却のリードタイム（occurred_at - submitted_at）を計算できる"
        );
    }

    #[tokio::test]
    async fn rejected_returns_change_nothing_and_emit_nothing() {
        let use_case = use_case();
        let before = use_case.videos().all();

        for (command, expected) in [
            (
                command(OTHER_COACH, TENNIS_VIDEO),
                DomainError::NotTheVideoCoach,
            ),
            // Gen1 はワンショット決済を実装しないので、ゴルフの動画の添削には着手できない（不変条件 2）
            (
                command(COACH, GOLF_VIDEO),
                DomainError::VideoReviewNotAllowed { sport: Sport::Golf },
            ),
        ] {
            let result = use_case.execute(command).await;
            assert!(
                matches!(&result, Err(ReturnVideoReviewError::Invalid(error)) if *error == expected),
                "{result:?}"
            );
        }
        let result = use_case
            .execute(command(COACH, VideoId::from_u128(99)))
            .await;
        assert!(
            matches!(result, Err(ReturnVideoReviewError::VideoNotFound)),
            "{result:?}"
        );
        assert_eq!(use_case.videos().all(), before);
        assert!(use_case.analytics().events().is_empty());
    }

    #[tokio::test]
    async fn a_review_is_returned_only_once() {
        let use_case = use_case();
        use_case
            .execute(command(COACH, TENNIS_VIDEO))
            .await
            .unwrap();
        let result = use_case.execute(command(COACH, TENNIS_VIDEO)).await;
        assert!(
            matches!(
                result,
                Err(ReturnVideoReviewError::Invalid(
                    DomainError::VideoReviewAlreadyReturned
                ))
            ),
            "{result:?}"
        );
        assert_eq!(use_case.analytics().events().len(), 1);
    }
}
