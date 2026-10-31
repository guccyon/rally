use domain::{
    DomainError,
    async_lesson::{AiFeedback, Video, VideoContentType},
    catalog::LessonMenuId,
    identity::{AccountId, Sport},
};

use uuid::Uuid;

use super::{
    AiFeedbackGenerator, AiFeedbackRequest, VideoLessonMenuLookup, VideoRepository, VideoStorage,
    video_id_uuid,
};
use crate::{
    Unavailable,
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
    domain_event::{DomainEvent, DomainEventPublisher},
};

/// 動画の送付の入力。`student_id` は認証した生徒のアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitVideoCommand {
    pub student_id: AccountId,
    pub coach_id: AccountId,
    pub sport: Sport,
    pub content_type: VideoContentType,
    /// 送った生徒の一言（コーチへの相談内容）。AI の一次フィードバックの材料に渡すだけで、保存しない
    pub note: Option<String>,
    /// 添削を頼むコーチの非同期レッスンのレッスンメニュー（任意。なければメニューなしの動画）
    pub lesson_menu_id: Option<LessonMenuId>,
}

/// 送付した動画と、動画本体を PUT する URL（保存先が発行する）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedVideo {
    pub video: Video,
    pub upload_url: String,
    /// AI の一次フィードバックを付けられなかった理由（付けられたら `None`）。送付は成功しているので、
    /// 外の層（interface）がログに書く
    pub ai_feedback_failure: Option<String>,
}

#[derive(Debug)]
pub enum SubmitVideoError {
    /// 選んだレッスンメニューが動画に合わない（送り先のコーチの・公開中の・非同期の・同じスポーツのメニューでない）
    Invalid(DomainError),
    /// 選んだレッスンメニューがない
    LessonMenuNotFound,
    /// 保存先・動画の保存先の障害
    Unavailable(Unavailable),
}

impl From<DomainError> for SubmitVideoError {
    fn from(error: DomainError) -> Self {
        Self::Invalid(error)
    }
}

/// 動画の送付のユースケース（student-app の `submitVideo`）。
///
/// domain の [`Video::submit`] で送付し、動画の保存先に本体の PUT 先を発行させ（[`VideoStorage`]）、動画を
/// 保存する。保存したら境界間イベント `VideoReviewRequested` を発行し（messaging はこれを購読して、その生徒とコーチの
/// スレッドに添削依頼のメッセージを足す。async-lesson は messaging を直接呼ばない）、行動イベント `video.submitted` を
/// 1 回発火する。送付の回数は数えない（月額は送り放題）。添削の有無・回数・状態は async-lesson だけが持つ。
///
/// 保存した直後に同じ処理の中で AI の一次フィードバックを作らせ（[`AiFeedbackGenerator`]）、動画に付けて保存する
/// （作った時刻は [`Clock`] の今）。AI や保存先が失敗しても送付は成功させ、一次フィードバックは空のままにする
/// （失敗の理由は [`SubmittedVideo::ai_feedback_failure`] で返し、書くのは外の層）。添削の着手条件・返却の流れには関わらない。
///
/// レッスンメニューの指定があれば、catalog に問い合わせて（[`VideoLessonMenuLookup`]）動画で選べるか
/// （[`Video::with_lesson_menu`]）を確かめ、送った時点の ID・名前を動画に写す。選べなければ何も保存せず、
/// イベントも発火しない。
///
/// 添削に着手してよいか（ゴルフはワンショット決済が確定した動画だけ）は返却のときに問い合わせる
/// （[`super::ReturnVideoReview`]）。
pub struct SubmitVideo<V, M, S, F, A, P, C> {
    videos: V,
    lesson_menus: M,
    storage: S,
    ai_feedback: F,
    analytics: A,
    events: P,
    clock: C,
}

impl<V, M, S, F, A, P, C> SubmitVideo<V, M, S, F, A, P, C>
where
    V: VideoRepository,
    M: VideoLessonMenuLookup,
    S: VideoStorage,
    F: AiFeedbackGenerator,
    A: AnalyticsSink,
    P: DomainEventPublisher,
    C: Clock,
{
    pub fn new(
        videos: V,
        lesson_menus: M,
        storage: S,
        ai_feedback: F,
        analytics: A,
        events: P,
        clock: C,
    ) -> Self {
        Self {
            videos,
            lesson_menus,
            storage,
            ai_feedback,
            analytics,
            events,
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
        let video = match command.lesson_menu_id {
            None => video,
            Some(id) => {
                let menu = self
                    .lesson_menus
                    .find_lesson_menu(id)
                    .await
                    .map_err(SubmitVideoError::Unavailable)?
                    .ok_or(SubmitVideoError::LessonMenuNotFound)?;
                video.with_lesson_menu(&menu)?
            }
        };
        let upload_url = self
            .storage
            .issue_upload_url(video.id(), command.content_type)
            .await
            .map_err(SubmitVideoError::Unavailable)?;
        self.videos
            .insert(video.clone())
            .await
            .map_err(SubmitVideoError::Unavailable)?;

        self.events
            .publish(DomainEvent::VideoReviewRequested {
                event_id: Uuid::new_v4(),
                occurred_at: now,
                student_id: video.student_id(),
                coach_id: video.coach_id(),
                video_id: video.id(),
            })
            .await;
        self.analytics
            .emit(AnalyticsEvent::VideoSubmitted {
                student_id: video.student_id(),
                coach_id: video.coach_id(),
                video_id: video.id(),
                sport: video.sport(),
                occurred_at: now,
            })
            .await;

        let request = AiFeedbackRequest {
            video_id: video.id(),
            sport: video.sport(),
            content_type: command.content_type,
            note: command.note,
        };
        let (video, ai_feedback_failure) = self.attach_ai_feedback(video, &request).await;
        Ok(SubmittedVideo {
            video,
            upload_url,
            ai_feedback_failure,
        })
    }

    /// 保存した動画 `video` に AI の一次フィードバックを付けて保存し、付けた動画を返す。どこかで失敗したら
    /// `video` をそのまま、失敗の理由と一緒に返す（送付は失敗させない）。
    async fn attach_ai_feedback(
        &self,
        video: Video,
        request: &AiFeedbackRequest,
    ) -> (Video, Option<String>) {
        let failed = |reason: &dyn std::fmt::Display| {
            Some(format!(
                "動画 {} の AI の一次フィードバックを付けられない（一次フィードバックなしで続ける）: {reason}",
                video_id_uuid::format(video.id())
            ))
        };
        let feedback = match self.ai_feedback.generate(request).await {
            Ok(comment) => AiFeedback::new(comment, self.clock.now()),
            Err(error) => return (video.clone(), failed(&error)),
        };
        let feedback = match feedback {
            Ok(feedback) => feedback,
            Err(error) => return (video.clone(), failed(&error)),
        };
        match self
            .videos
            .update(video.id(), |v| v.record_ai_feedback(feedback))
            .await
        {
            Ok(updated) => (updated, None),
            Err(error) => {
                let reason = failed(&format!("{error:?}"));
                (video, reason)
            }
        }
    }

    pub fn videos(&self) -> &V {
        &self.videos
    }

    pub fn analytics(&self) -> &A {
        &self.analytics
    }

    pub fn events(&self) -> &P {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use std::sync::{Arc, Mutex};

    use domain::{
        async_lesson::{VideoLessonMenu, VideoStatus},
        catalog::{
            LessonDuration, LessonFormat, LessonMenu, LessonMenuContent, LessonMenuDescription,
            LessonMenuName,
        },
    };

    use super::*;
    use crate::{
        catalog::CatalogLessonMenuLookup,
        in_memory::{
            FixedAiFeedbackGenerator, FixedClock, FixedVideoStorage, InMemoryLessonMenuRepository,
            InMemoryVideoRepository, RecordingAnalyticsSink, RecordingDomainEventPublisher,
        },
    };

    type Menus = CatalogLessonMenuLookup<InMemoryLessonMenuRepository>;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn use_case() -> SubmitVideo<
        InMemoryVideoRepository,
        Menus,
        FixedVideoStorage,
        FixedAiFeedbackGenerator,
        RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
        FixedClock,
    > {
        use_case_with(FixedAiFeedbackGenerator)
    }

    fn use_case_with<F: AiFeedbackGenerator>(
        ai_feedback: F,
    ) -> SubmitVideo<
        InMemoryVideoRepository,
        Menus,
        FixedVideoStorage,
        F,
        RecordingAnalyticsSink,
        RecordingDomainEventPublisher,
        FixedClock,
    > {
        SubmitVideo::new(
            InMemoryVideoRepository::default(),
            menus(),
            FixedVideoStorage,
            ai_feedback,
            RecordingAnalyticsSink::default(),
            RecordingDomainEventPublisher::default(),
            FixedClock(now()),
        )
    }

    /// 受け取った材料を記録し、`reply` を返す [`AiFeedbackGenerator`]。
    #[derive(Clone)]
    struct ScriptedAiFeedback {
        reply: fn() -> Result<String, Unavailable>,
        asked: Arc<Mutex<Vec<AiFeedbackRequest>>>,
    }

    impl ScriptedAiFeedback {
        fn new(reply: fn() -> Result<String, Unavailable>) -> Self {
            Self {
                reply,
                asked: Arc::default(),
            }
        }
    }

    impl AiFeedbackGenerator for ScriptedAiFeedback {
        async fn generate(&self, request: &AiFeedbackRequest) -> Result<String, Unavailable> {
            self.asked.lock().unwrap().push(request.clone());
            (self.reply)()
        }
    }

    fn command() -> SubmitVideoCommand {
        SubmitVideoCommand {
            student_id: STUDENT,
            coach_id: COACH,
            sport: Sport::Tennis,
            content_type: VideoContentType::Mp4,
            note: Some("フォアハンドを見てほしい".to_string()),
            lesson_menu_id: None,
        }
    }

    /// `COACH` のテニスの公開中の非同期のメニュー
    const MENU: LessonMenuId = LessonMenuId::from_u128(50);
    /// `COACH` のテニスの非公開の非同期のメニュー
    const UNPUBLISHED_MENU: LessonMenuId = LessonMenuId::from_u128(51);
    /// `COACH` のテニスの公開中の 60 分の同期のメニュー
    const LIVE_MENU: LessonMenuId = LessonMenuId::from_u128(52);
    /// 別のコーチのテニスの公開中の非同期のメニュー
    const OTHER_COACH_MENU: LessonMenuId = LessonMenuId::from_u128(53);
    /// `COACH` のサッカーの公開中の非同期のメニュー
    const SOCCER_MENU: LessonMenuId = LessonMenuId::from_u128(54);

    fn menu(
        id: LessonMenuId,
        coach_id: AccountId,
        sport: Sport,
        format: LessonFormat,
        is_published: bool,
    ) -> LessonMenu {
        LessonMenu::restore(
            id,
            coach_id,
            LessonMenuContent {
                sport,
                format,
                name: LessonMenuName::new("フォーム添削").unwrap(),
                description: LessonMenuDescription::default(),
            },
            is_published,
            now(),
        )
    }

    fn menus() -> Menus {
        let sixty = LessonFormat::Live(LessonDuration::from_minutes(60).unwrap());
        CatalogLessonMenuLookup::new(InMemoryLessonMenuRepository::with_menus([
            menu(MENU, COACH, Sport::Tennis, LessonFormat::Async, true),
            menu(
                UNPUBLISHED_MENU,
                COACH,
                Sport::Tennis,
                LessonFormat::Async,
                false,
            ),
            menu(LIVE_MENU, COACH, Sport::Tennis, sixty, true),
            menu(
                OTHER_COACH_MENU,
                AccountId::from_u128(8),
                Sport::Tennis,
                LessonFormat::Async,
                true,
            ),
            menu(SOCCER_MENU, COACH, Sport::Soccer, LessonFormat::Async, true),
        ]))
    }

    fn command_with_menu(lesson_menu_id: LessonMenuId) -> SubmitVideoCommand {
        SubmitVideoCommand {
            lesson_menu_id: Some(lesson_menu_id),
            ..command()
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
        assert_eq!(
            video.lesson_menu(),
            None,
            "メニューを選ばなければメニューなし"
        );
        assert_eq!(
            video.ai_feedback(),
            Some(&AiFeedback::new(FixedAiFeedbackGenerator::COMMENT, now()).unwrap())
        );
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

        // 境界間イベント（messaging はこれでスレッドに添削依頼のメッセージを足す）
        let published = use_case.events().events();
        assert_eq!(published.len(), 1);
        assert!(
            matches!(
                published[0],
                DomainEvent::VideoReviewRequested {
                    occurred_at,
                    student_id: STUDENT,
                    coach_id: COACH,
                    video_id,
                    ..
                } if occurred_at == now() && video_id == video.id()
            ),
            "{published:?}"
        );
        assert_eq!(published[0].name(), "VideoReviewRequested");
    }

    #[tokio::test]
    async fn monthly_students_can_submit_as_many_videos_as_they_like() {
        let use_case = use_case();
        for _ in 0..5 {
            use_case.execute(command()).await.unwrap();
        }
        assert_eq!(use_case.videos().all().len(), 5);
        assert_eq!(use_case.analytics().events().len(), 5);
        assert_eq!(use_case.events().events().len(), 5);
    }

    #[tokio::test]
    async fn the_ai_feedback_is_made_from_the_video_and_the_students_note_and_saved() {
        let ai = ScriptedAiFeedback::new(|| Ok("ラケットの面が上を向いています".to_string()));
        let use_case = use_case_with(ai.clone());

        let submitted = use_case.execute(command()).await.unwrap();

        let video = &submitted.video;
        assert_eq!(
            *ai.asked.lock().unwrap(),
            [AiFeedbackRequest {
                video_id: video.id(),
                sport: Sport::Tennis,
                content_type: VideoContentType::Mp4,
                note: Some("フォアハンドを見てほしい".to_string()),
            }]
        );
        let expected = AiFeedback::new("ラケットの面が上を向いています", now()).unwrap();
        assert_eq!(video.ai_feedback(), Some(&expected));
        assert_eq!(
            use_case.videos().all()[0].ai_feedback(),
            Some(&expected),
            "保存した動画に付いている"
        );
        assert_eq!(video.review(), None, "コーチの添削とは別の項目");
        assert_eq!(submitted.ai_feedback_failure, None);
    }

    #[tokio::test]
    async fn submitting_succeeds_without_ai_feedback_when_the_ai_fails() {
        for reply in [
            (|| Err(Unavailable::new("AI が応答しない"))) as fn() -> Result<String, Unavailable>,
            || Ok("  ".to_string()),
        ] {
            let use_case = use_case_with(ScriptedAiFeedback::new(reply));

            let submitted = use_case.execute(command()).await.unwrap();

            assert_eq!(submitted.video.ai_feedback(), None);
            assert!(
                submitted.ai_feedback_failure.is_some(),
                "失敗の理由は外の層に返す（application は書かない）"
            );
            assert_eq!(use_case.videos().all(), vec![submitted.video.clone()]);
            assert_eq!(submitted.video.status(), VideoStatus::Submitted);
            assert_eq!(
                use_case.analytics().events().len(),
                1,
                "video.submitted は発火する"
            );
        }
    }

    #[tokio::test]
    async fn the_chosen_async_menu_of_the_coach_is_copied_to_the_video() {
        let use_case = use_case();

        let submitted = use_case.execute(command_with_menu(MENU)).await.unwrap();

        let copy =
            VideoLessonMenu::copy_of(&menu(MENU, COACH, Sport::Tennis, LessonFormat::Async, true));
        assert_eq!(submitted.video.lesson_menu(), Some(&copy));
        assert_eq!(
            use_case.videos().all()[0].lesson_menu(),
            Some(&copy),
            "保存した動画にも写しがある"
        );
        assert_eq!(use_case.analytics().events().len(), 1);
        assert_eq!(use_case.events().events().len(), 1);
    }

    #[tokio::test]
    async fn a_menu_that_cannot_be_chosen_rejects_the_submission_and_saves_nothing() {
        for (lesson_menu_id, expected) in [
            (
                OTHER_COACH_MENU,
                Some(DomainError::VideoLessonMenuOfAnotherCoach),
            ),
            (
                UNPUBLISHED_MENU,
                Some(DomainError::VideoLessonMenuNotPublished),
            ),
            (LIVE_MENU, Some(DomainError::LessonMenuNotAsync)),
            (
                SOCCER_MENU,
                Some(DomainError::VideoLessonMenuSportMismatch {
                    menu: Sport::Soccer,
                    video: Sport::Tennis,
                }),
            ),
            (LessonMenuId::from_u128(99), None),
        ] {
            let use_case = use_case();

            let error = use_case
                .execute(command_with_menu(lesson_menu_id))
                .await
                .unwrap_err();

            match (error, expected) {
                (SubmitVideoError::Invalid(error), Some(expected)) => assert_eq!(error, expected),
                (SubmitVideoError::LessonMenuNotFound, None) => {}
                (error, expected) => panic!("{error:?} / {expected:?}"),
            }
            assert!(use_case.videos().all().is_empty(), "動画は保存しない");
            assert!(use_case.analytics().events().is_empty());
            assert!(use_case.events().events().is_empty());
        }
    }
}
