use domain::{
    async_lesson::{Video, VideoContentType},
    identity::{AccountId, Sport},
};

use super::{VideoRepository, VideoStorage};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
};

/// 動画の送付の入力。`student_id` は認証した生徒のアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitVideoCommand {
    pub student_id: AccountId,
    pub coach_id: AccountId,
    pub sport: Sport,
    pub content_type: VideoContentType,
}

/// 送付した動画と、動画本体を PUT する URL（保存先が発行する）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedVideo {
    pub video: Video,
    pub upload_url: String,
}

#[derive(Debug)]
pub enum SubmitVideoError {
    /// 保存先・動画の保存先の障害
    Unavailable(Unavailable),
}

/// 動画の送付のユースケース（student-app の `submitVideo`）。
///
/// domain の [`Video::submit`] で送付し、動画の保存先に本体の PUT 先を発行させ（[`VideoStorage`]）、動画を
/// 保存する。保存したら行動イベント `video.submitted` を 1 回発火する。送付の回数は数えない（月額は送り放題）。
/// 添削に着手してよいか（ゴルフはワンショット決済が確定した動画だけ）は返却のときに問い合わせる
/// （[`super::ReturnVideoReview`]）。
pub struct SubmitVideo<V, S, A, C> {
    videos: V,
    storage: S,
    analytics: A,
    clock: C,
}

impl<V, S, A, C> SubmitVideo<V, S, A, C>
where
    V: VideoRepository,
    S: VideoStorage,
    A: AnalyticsSink,
    C: Clock,
{
    pub fn new(videos: V, storage: S, analytics: A, clock: C) -> Self {
        Self {
            videos,
            storage,
            analytics,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: SubmitVideoCommand,
    ) -> Result<SubmittedVideo, SubmitVideoError> {
        let now = self.clock.now();
        let video = Video::submit(
            self.videos.next_video_id(),
            command.student_id,
            command.coach_id,
            command.sport,
            now,
        );
        let upload_url = self
            .storage
            .issue_upload_url(video.id(), command.content_type)
            .await
            .map_err(SubmitVideoError::Unavailable)?;
        self.videos
            .insert(video.clone())
            .await
            .map_err(SubmitVideoError::Unavailable)?;

        self.analytics
            .emit(AnalyticsEvent::VideoSubmitted {
                student_id: video.student_id(),
                coach_id: video.coach_id(),
                video_id: video.id(),
                sport: video.sport(),
                occurred_at: now,
            })
            .await;

        Ok(SubmittedVideo { video, upload_url })
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

    use domain::async_lesson::VideoStatus;

    use super::*;
    use crate::{
        async_lesson::video_id_uuid,
        in_memory::{
            FixedClock, FixedVideoStorage, InMemoryVideoRepository, RecordingAnalyticsSink,
        },
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn use_case()
    -> SubmitVideo<InMemoryVideoRepository, FixedVideoStorage, RecordingAnalyticsSink, FixedClock>
    {
        SubmitVideo::new(
            InMemoryVideoRepository::default(),
            FixedVideoStorage,
            RecordingAnalyticsSink::default(),
            FixedClock(now()),
        )
    }

    fn command() -> SubmitVideoCommand {
        SubmitVideoCommand {
            student_id: STUDENT,
            coach_id: COACH,
            sport: Sport::Tennis,
            content_type: VideoContentType::Mp4,
        }
    }

    #[tokio::test]
    async fn submitting_saves_the_video_issues_an_upload_url_and_emits_video_submitted() {
        let use_case = use_case();

        let submitted = use_case.execute(command()).await.unwrap();

        let video = &submitted.video;
        assert_eq!(
            (video.student_id(), video.coach_id(), video.sport()),
            (STUDENT, COACH, Sport::Tennis)
        );
        assert_eq!(video.status(), VideoStatus::Submitted);
        assert_eq!(video.submitted_at(), now());
        assert_eq!(use_case.videos().all(), vec![video.clone()]);
        assert_eq!(
            submitted.upload_url,
            format!(
                "https://fixed-storage.invalid/videos/{}",
                video_id_uuid::format(video.id())
            )
        );

        let analytics = use_case.analytics().events();
        assert_eq!(
            analytics,
            vec![AnalyticsEvent::VideoSubmitted {
                student_id: STUDENT,
                coach_id: COACH,
                video_id: video.id(),
                sport: Sport::Tennis,
                occurred_at: now(),
            }]
        );
        assert_eq!(analytics[0].name(), "video.submitted");
    }

    #[tokio::test]
    async fn monthly_students_can_submit_as_many_videos_as_they_like() {
        let use_case = use_case();
        for _ in 0..5 {
            use_case.execute(command()).await.unwrap();
        }
        assert_eq!(use_case.videos().all().len(), 5);
        assert_eq!(use_case.analytics().events().len(), 5);
    }
}
