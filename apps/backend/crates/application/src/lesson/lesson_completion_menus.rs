use domain::lesson::{LessonCompletion, LessonCompletionMenu, LessonSource};

use crate::{
    Unavailable,
    async_lesson::VideoRepository,
    live_lesson::{BookingLookup, LiveLessonSessionRepository},
};

/// 完了したレッスンのメニューを、完了記録の元（[`LessonSource`]）から読む。読むだけで、何も変えない。
///
/// 同期レッスンは通話セッションに結び付いたその生徒の予約（scheduling の予約は live-lesson の [`BookingLookup`] で
/// 読む）、非同期レッスンは動画が持つメニューの写しを [`LessonCompletionMenu`] に写す（予約・動画の写しから完了した
/// レッスンのメニューへの変換はここだけ）。review の評価の一覧（[`super::LessonCompletionMenuLookup`]）と生徒の
/// 受けたレッスンの一覧（[`super::ListMyLessonCompletions`]）が共有する。
#[derive(Debug, Clone)]
pub(crate) struct LessonCompletionMenus<L, B, W> {
    sessions: L,
    bookings: B,
    videos: W,
}

impl<L, B, W> LessonCompletionMenus<L, B, W> {
    pub(crate) fn new(sessions: L, bookings: B, videos: W) -> Self {
        Self {
            sessions,
            bookings,
            videos,
        }
    }
}

impl<L, B, W> LessonCompletionMenus<L, B, W>
where
    L: LiveLessonSessionRepository,
    B: BookingLookup,
    W: VideoRepository,
{
    /// `completion` の元が持つメニューの写し。元がない・メニューを持たないなら `None`。
    pub(crate) async fn find(
        &self,
        completion: &LessonCompletion,
    ) -> Result<Option<LessonCompletionMenu>, Unavailable> {
        match completion.source() {
            LessonSource::Live(session_id) => {
                let Some(booking_id) = self
                    .sessions
                    .find(session_id)
                    .await?
                    .and_then(|session| session.booking_of(completion.student_id()))
                else {
                    return Ok(None);
                };
                Ok(self
                    .bookings
                    .find_booking(booking_id)
                    .await?
                    .and_then(|booking| {
                        booking
                            .lesson_menu()
                            .map(|menu| LessonCompletionMenu::new(menu.id(), menu.name().clone()))
                    }))
            }
            LessonSource::Async(video_id) => {
                Ok(self.videos.find(video_id).await?.and_then(|video| {
                    video
                        .lesson_menu()
                        .map(|menu| LessonCompletionMenu::new(menu.id(), menu.name().clone()))
                }))
            }
        }
    }
}
