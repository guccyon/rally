use domain::{
    lesson::{LessonCompletionId, LessonSource},
    review::ReviewLessonMenu,
};

use super::LessonCompletionRepository;
use crate::{
    Unavailable,
    async_lesson::VideoRepository,
    live_lesson::{BookingLookup, LiveLessonSessionRepository},
    review::ReviewedLessonMenuLookup,
};

/// review の [`ReviewedLessonMenuLookup`] の、完了したレッスンの元による実装。読むだけで、何も変えない。
///
/// 完了記録の元（[`LessonSource`]）をたどり、同期レッスンは通話セッションに結び付いたその生徒の予約
/// （scheduling の予約は live-lesson の [`BookingLookup`] で読む）、非同期レッスンは動画が持つメニューの写しを、
/// review の [`ReviewLessonMenu`] に写す（予約・動画の写しから評価のメニューへの変換はここだけ）。
#[derive(Debug, Clone)]
pub struct LessonCompletionMenuLookup<M, L, B, W> {
    completions: M,
    sessions: L,
    bookings: B,
    videos: W,
}

impl<M, L, B, W> LessonCompletionMenuLookup<M, L, B, W> {
    pub fn new(completions: M, sessions: L, bookings: B, videos: W) -> Self {
        Self {
            completions,
            sessions,
            bookings,
            videos,
        }
    }
}

impl<M, L, B, W> ReviewedLessonMenuLookup for LessonCompletionMenuLookup<M, L, B, W>
where
    M: LessonCompletionRepository,
    L: LiveLessonSessionRepository,
    B: BookingLookup,
    W: VideoRepository,
{
    async fn find_lesson_menu(
        &self,
        lesson_id: LessonCompletionId,
    ) -> Result<Option<ReviewLessonMenu>, Unavailable> {
        let Some(completion) = self.completions.find(lesson_id).await? else {
            return Ok(None);
        };
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
                            .map(|menu| ReviewLessonMenu::new(menu.id(), menu.name().clone()))
                    }))
            }
            LessonSource::Async(video_id) => {
                Ok(self.videos.find(video_id).await?.and_then(|video| {
                    video
                        .lesson_menu()
                        .map(|menu| ReviewLessonMenu::new(menu.id(), menu.name().clone()))
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{ReviewComment, Video, VideoId, VideoLessonMenu, VideoReviewId},
        catalog::Sport,
        catalog::{LessonDuration, LessonMenuId, LessonMenuName},
        identity::AccountId,
        lesson::LessonCompletion,
        live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
        scheduling::{Booking, BookingId, BookingLessonMenu, Slot, SlotCapacity, SlotId},
    };

    use super::*;
    use crate::in_memory::{
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemoryVideoRepository,
    };

    const COACH: AccountId = AccountId::from_u128(7);
    /// グループレッスンの 3 人。予約のメニューは生徒 1 と 2 だけが持つ（予約した時点の写しなので人によって違いうる）
    const STUDENTS: [AccountId; 3] = [
        AccountId::from_u128(1),
        AccountId::from_u128(2),
        AccountId::from_u128(3),
    ];

    fn at(hours: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hours * 3600)
    }

    /// 予約の保存先の代わり（読むだけ）。
    struct FixedBookings(Vec<Booking>);

    impl BookingLookup for FixedBookings {
        async fn find_booking(
            &self,
            booking_id: BookingId,
        ) -> Result<Option<Booking>, Unavailable> {
            Ok(self.0.iter().find(|b| b.id() == booking_id).cloned())
        }

        async fn find_slot_bookings(&self, booking: &Booking) -> Result<Vec<Booking>, Unavailable> {
            Ok(self
                .0
                .iter()
                .filter(|b| b.slot_id() == booking.slot_id())
                .cloned()
                .collect())
        }
    }

    type Lookup = LessonCompletionMenuLookup<
        InMemoryLessonCompletionRepository,
        InMemoryLiveLessonSessionRepository,
        FixedBookings,
        InMemoryVideoRepository,
    >;

    fn name(text: &str) -> LessonMenuName {
        LessonMenuName::new(text).unwrap()
    }

    /// 定員 3 の枠の、確定した 3 人の予約。生徒 1 は「フォア改善」、生徒 2 は「サーブ」のメニューの写しを持つ。
    fn group_bookings() -> Vec<Booking> {
        let mut slot = Slot::publish(
            SlotId::from_u128(10),
            COACH,
            Sport::Tennis,
            at(10),
            at(11),
            SlotCapacity::new(3).unwrap(),
        )
        .unwrap();
        let menus = [Some(("フォア改善", 21)), Some(("サーブ", 22)), None];
        let mut bookings: Vec<Booking> = Vec::new();
        for (i, (student, menu)) in STUDENTS.into_iter().zip(menus).enumerate() {
            let mut booking = Booking::reserve(
                BookingId::from_u128(30 + i as u128),
                student,
                false,
                &mut slot,
                &bookings,
                at(0),
            )
            .unwrap()
            .with_restored_lesson_menu(menu.map(|(text, id)| {
                BookingLessonMenu::restore(
                    LessonMenuId::from_u128(id),
                    name(text),
                    Some(LessonDuration::from_minutes(60).unwrap()),
                )
            }));
            booking.confirm(student, &mut slot, at(0)).unwrap();
            bookings.push(booking);
        }
        bookings
    }

    /// 返却した動画（メニューは `menu`）。
    fn reviewed_video(id: u128, menu: Option<VideoLessonMenu>) -> Video {
        let mut video = Video::restore(
            VideoId::from_u128(id),
            STUDENTS[0],
            COACH,
            Sport::Tennis,
            at(0),
            None,
            menu,
        )
        .unwrap();
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
    }

    /// グループレッスンの 3 人と、メニューのある動画・ない動画の完了したレッスンを記録した照会と、それぞれの完了記録
    /// （生徒 1・2・3 の同期、メニューのある動画、ない動画の順）。
    async fn lookup() -> (Lookup, Vec<LessonCompletion>) {
        let bookings = group_bookings();
        let mut session = LiveLessonSession::start(
            SessionId::from_u128(50),
            COACH,
            &bookings[0],
            &bookings,
            None,
            at(10),
        )
        .unwrap();
        session
            .end(
                COACH,
                at(11),
                ConnectionQuality::new(0.5, 12, 80, 0).unwrap(),
            )
            .unwrap();
        let with_menu = reviewed_video(
            60,
            Some(VideoLessonMenu::restore(
                LessonMenuId::from_u128(23),
                name("フォーム添削"),
            )),
        );
        let without_menu = reviewed_video(61, None);

        let completions = InMemoryLessonCompletionRepository::default();
        let mut recorded = Vec::new();
        for student in STUDENTS {
            let id = completions.next_id();
            recorded.push(
                completions
                    .record(student, |history| {
                        session.complete_for(id, COACH, student, history, at(12))
                    })
                    .await
                    .unwrap(),
            );
        }
        for video in [&with_menu, &without_menu] {
            let id = completions.next_id();
            recorded.push(
                completions
                    .record(STUDENTS[0], |history| {
                        video.complete(id, STUDENTS[0], history, at(12))
                    })
                    .await
                    .unwrap(),
            );
        }
        let lookup = LessonCompletionMenuLookup::new(
            completions,
            InMemoryLiveLessonSessionRepository::with_sessions([session]),
            FixedBookings(bookings),
            InMemoryVideoRepository::with_videos([with_menu, without_menu]),
        );
        (lookup, recorded)
    }

    #[tokio::test]
    async fn a_live_lesson_has_the_menu_of_the_booking_of_its_student() {
        let (lookup, completions) = lookup().await;
        let menus = [
            Some(ReviewLessonMenu::new(
                LessonMenuId::from_u128(21),
                name("フォア改善"),
            )),
            // グループレッスンでも、開始した予約ではなくその生徒の予約の写し
            Some(ReviewLessonMenu::new(
                LessonMenuId::from_u128(22),
                name("サーブ"),
            )),
            None,
        ];
        for (completion, menu) in completions.iter().zip(menus) {
            assert_eq!(
                lookup.find_lesson_menu(completion.id()).await.unwrap(),
                menu,
                "{:?}",
                completion.student_id()
            );
        }
    }

    #[tokio::test]
    async fn an_async_lesson_has_the_menu_of_its_video() {
        let (lookup, completions) = lookup().await;
        assert_eq!(
            lookup.find_lesson_menu(completions[3].id()).await.unwrap(),
            Some(ReviewLessonMenu::new(
                LessonMenuId::from_u128(23),
                name("フォーム添削")
            ))
        );
        assert_eq!(
            lookup.find_lesson_menu(completions[4].id()).await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn an_unknown_lesson_has_no_menu() {
        let (lookup, _) = lookup().await;
        assert_eq!(
            lookup
                .find_lesson_menu(LessonCompletionId::from_u128(99))
                .await
                .unwrap(),
            None
        );
    }
}
