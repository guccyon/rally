use domain::{
    catalog::Sport,
    lesson::{LessonCompletionId, LessonSource},
};

use super::LessonCompletionRepository;
use crate::{
    Unavailable,
    async_lesson::VideoRepository,
    live_lesson::{BookingLookup, LiveLessonSessionRepository},
    review::ReviewedLessonSportLookup,
};

/// review の [`ReviewedLessonSportLookup`] の、完了したレッスンの元による実装。読むだけで、何も変えない。
///
/// 完了したレッスンのスポーツは元のスポーツ（同期レッスンは通話セッションに結び付いたその生徒の予約の枠、非同期レッスンは
/// 動画）。scheduling の予約と枠は live-lesson の [`BookingLookup`] で読む。
#[derive(Debug, Clone)]
pub struct LessonCompletionSportLookup<M, L, B, W> {
    completions: M,
    sessions: L,
    bookings: B,
    videos: W,
}

impl<M, L, B, W> LessonCompletionSportLookup<M, L, B, W> {
    pub fn new(completions: M, sessions: L, bookings: B, videos: W) -> Self {
        Self {
            completions,
            sessions,
            bookings,
            videos,
        }
    }
}

impl<M, L, B, W> ReviewedLessonSportLookup for LessonCompletionSportLookup<M, L, B, W>
where
    M: LessonCompletionRepository,
    L: LiveLessonSessionRepository,
    B: BookingLookup,
    W: VideoRepository,
{
    async fn find_lesson_sport(
        &self,
        lesson_id: LessonCompletionId,
    ) -> Result<Option<Sport>, Unavailable> {
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
                let Some(booking) = self.bookings.find_booking(booking_id).await? else {
                    return Ok(None);
                };
                Ok(self
                    .bookings
                    .find_slot(booking.slot_id())
                    .await?
                    .map(|slot| slot.sport()))
            }
            LessonSource::Async(video_id) => {
                Ok(self.videos.find(video_id).await?.map(|video| video.sport()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{ReviewComment, Video, VideoId, VideoReviewId},
        identity::AccountId,
        lesson::LessonCompletion,
        live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
        scheduling::{Booking, BookingId, Slot, SlotCapacity, SlotId},
    };

    use super::*;
    use crate::in_memory::{
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemoryVideoRepository,
    };

    const COACH: AccountId = AccountId::from_u128(7);
    const STUDENT: AccountId = AccountId::from_u128(1);

    fn at(hours: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hours * 3600)
    }

    /// 予約と枠の保存先の代わり（読むだけ）。
    struct FixedBookings {
        slot: Slot,
        bookings: Vec<Booking>,
    }

    impl BookingLookup for FixedBookings {
        async fn find_booking(
            &self,
            booking_id: BookingId,
        ) -> Result<Option<Booking>, Unavailable> {
            Ok(self.bookings.iter().find(|b| b.id() == booking_id).cloned())
        }

        async fn find_slot_bookings(&self, booking: &Booking) -> Result<Vec<Booking>, Unavailable> {
            Ok(self
                .bookings
                .iter()
                .filter(|b| b.slot_id() == booking.slot_id())
                .cloned()
                .collect())
        }

        async fn find_slot(&self, slot_id: SlotId) -> Result<Option<Slot>, Unavailable> {
            Ok((self.slot.id() == slot_id).then(|| self.slot.clone()))
        }
    }

    type Lookup = LessonCompletionSportLookup<
        InMemoryLessonCompletionRepository,
        InMemoryLiveLessonSessionRepository,
        FixedBookings,
        InMemoryVideoRepository,
    >;

    /// サッカーの枠の確定した予約で通話したレッスンと、野球の動画を返却した添削のレッスンを記録した照会と、それぞれの
    /// 完了記録（同期、非同期の順）。
    async fn lookup() -> (Lookup, Vec<LessonCompletion>) {
        let mut slot = Slot::publish(
            SlotId::from_u128(10),
            COACH,
            Sport::Soccer,
            at(10),
            at(11),
            SlotCapacity::new(1).unwrap(),
        )
        .unwrap();
        let mut booking = Booking::reserve(
            BookingId::from_u128(30),
            STUDENT,
            false,
            &mut slot,
            &[],
            at(0),
        )
        .unwrap();
        booking.confirm(STUDENT, &mut slot, at(0)).unwrap();
        let mut session = LiveLessonSession::start(
            SessionId::from_u128(50),
            COACH,
            &booking,
            std::slice::from_ref(&booking),
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
        let mut video = Video::restore(
            VideoId::from_u128(60),
            STUDENT,
            COACH,
            Sport::Baseball,
            at(0),
            None,
            None,
        )
        .unwrap();
        video
            .return_review(
                VideoReviewId::from_u128(60),
                COACH,
                ReviewComment::new("よい").unwrap(),
                true,
                at(1),
            )
            .unwrap();

        let completions = InMemoryLessonCompletionRepository::default();
        let mut recorded = Vec::new();
        let id = completions.next_id();
        recorded.push(
            completions
                .record(STUDENT, |history| {
                    session.complete_for(id, COACH, STUDENT, history, at(12))
                })
                .await
                .unwrap(),
        );
        let id = completions.next_id();
        recorded.push(
            completions
                .record(STUDENT, |history| {
                    video.complete(id, STUDENT, history, at(12))
                })
                .await
                .unwrap(),
        );
        let lookup = LessonCompletionSportLookup::new(
            completions,
            InMemoryLiveLessonSessionRepository::with_sessions([session]),
            FixedBookings {
                slot,
                bookings: vec![booking],
            },
            InMemoryVideoRepository::with_videos([video]),
        );
        (lookup, recorded)
    }

    #[tokio::test]
    async fn a_live_lesson_has_the_sport_of_the_slot_of_its_booking() {
        let (lookup, completions) = lookup().await;
        assert_eq!(
            lookup.find_lesson_sport(completions[0].id()).await.unwrap(),
            Some(Sport::Soccer)
        );
    }

    #[tokio::test]
    async fn an_async_lesson_has_the_sport_of_its_video() {
        let (lookup, completions) = lookup().await;
        assert_eq!(
            lookup.find_lesson_sport(completions[1].id()).await.unwrap(),
            Some(Sport::Baseball)
        );
    }

    #[tokio::test]
    async fn an_unknown_lesson_has_no_sport() {
        let (lookup, _) = lookup().await;
        assert_eq!(
            lookup
                .find_lesson_sport(LessonCompletionId::from_u128(99))
                .await
                .unwrap(),
            None
        );
    }
}
