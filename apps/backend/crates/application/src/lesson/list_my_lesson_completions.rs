use std::{cmp::Reverse, collections::BTreeSet};

use domain::{
    identity::AccountId,
    lesson::{LessonCompletion, LessonCompletionId},
};

use super::{
    LessonCompletionCoachDisplayNameLookup, LessonCompletionRepository,
    LessonCompletionReviewLookup, MyLessonCompletion, MyLessonCompletionList,
    lesson_completion_menus::LessonCompletionMenus,
};
use crate::{
    Unavailable,
    async_lesson::VideoRepository,
    live_lesson::{BookingLookup, LiveLessonSessionRepository},
};

/// 生徒が受けたレッスンの一覧のユースケース（student-app の `listMyLessonCompletions`。生徒だけが呼ぶ）。
///
/// 生徒 `student_id` の完了したレッスンを、同期（通話）・非同期（動画添削）の両方とも、完了した時刻の新しい順に並べる
/// （同じ時刻は完了記録の ID の順）。各レッスンには、完了記録の元（予約・動画）が持つレッスンメニューの写し
/// （[`LessonCompletionMenus`]）と、生徒が評価を付けたか（review に [`LessonCompletionReviewLookup`] で問い合わせる）を
/// 添え、コーチの表示名は profile に問い合わせて（[`LessonCompletionCoachDisplayNameLookup`]）添える。ほかの
/// コンテキストの保存先は直接読まない。読むだけで何も変えない。
pub struct ListMyLessonCompletions<R, L, B, W, D, V> {
    completions: R,
    menus: LessonCompletionMenus<L, B, W>,
    coaches: D,
    reviews: V,
}

impl<R, L, B, W, D, V> ListMyLessonCompletions<R, L, B, W, D, V>
where
    R: LessonCompletionRepository,
    L: LiveLessonSessionRepository,
    B: BookingLookup,
    W: VideoRepository,
    D: LessonCompletionCoachDisplayNameLookup,
    V: LessonCompletionReviewLookup,
{
    pub fn new(
        completions: R,
        sessions: L,
        bookings: B,
        videos: W,
        coaches: D,
        reviews: V,
    ) -> Self {
        Self {
            completions,
            menus: LessonCompletionMenus::new(sessions, bookings, videos),
            coaches,
            reviews,
        }
    }

    pub async fn execute(
        &self,
        student_id: AccountId,
    ) -> Result<MyLessonCompletionList, Unavailable> {
        let mut completions = self.completions.completions_of_student(student_id).await?;
        if completions.is_empty() {
            return Ok(MyLessonCompletionList::default());
        }
        completions.sort_by_key(|completion| (Reverse(completion.completed_at()), completion.id()));

        let coach_ids: BTreeSet<AccountId> =
            completions.iter().map(LessonCompletion::coach_id).collect();
        let coaches = self.coaches.find_coach_display_names(&coach_ids).await?;
        let lesson_ids: BTreeSet<LessonCompletionId> =
            completions.iter().map(LessonCompletion::id).collect();
        let reviewed = self
            .reviews
            .find_reviewed_lessons(student_id, &lesson_ids)
            .await?;

        let mut list = Vec::with_capacity(completions.len());
        for completion in completions {
            list.push(MyLessonCompletion {
                lesson_menu: self.menus.find(&completion).await?,
                reviewed: reviewed.contains(&completion.id()),
                completion,
            });
        }
        Ok(MyLessonCompletionList {
            completions: list,
            coaches,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use domain::{
        async_lesson::{ReviewComment, Video, VideoId, VideoLessonMenu, VideoReviewId},
        catalog::{LessonDuration, LessonMenuId, LessonMenuName, Sport},
        lesson::{LessonCompletionMenu, LessonSource},
        live_lesson::{ConnectionQuality, LiveLessonSession, SessionId},
        profile::DisplayName,
        scheduling::{Booking, BookingId, BookingLessonMenu, Slot, SlotCapacity, SlotId},
    };

    use super::*;
    use crate::in_memory::{
        InMemoryLessonCompletionRepository, InMemoryLiveLessonSessionRepository,
        InMemoryVideoRepository,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    /// 表示名が引けないコーチ
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hours: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hours * 3600)
    }

    fn name(text: &str) -> LessonMenuName {
        LessonMenuName::new(text).unwrap()
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

    /// `COACH` の表示名だけを知っていて（`OTHER_COACH` は引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingCoaches {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl LessonCompletionCoachDisplayNameLookup for RecordingCoaches {
        async fn find_coach_display_names(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, DisplayName::new("たなか").unwrap()))
                .collect())
        }
    }

    /// 評価済みの問い合わせ（生徒と、問い合わせた完了したレッスン）の記録。
    type AskedReviews = Arc<Mutex<Vec<(AccountId, BTreeSet<LessonCompletionId>)>>>;

    /// 生徒ごとに評価を付けた完了したレッスンを持ち、問い合わせを記録する。
    #[derive(Clone, Default)]
    struct RecordingReviews {
        reviewed: Vec<(AccountId, LessonCompletionId)>,
        asked: AskedReviews,
    }

    impl LessonCompletionReviewLookup for RecordingReviews {
        async fn find_reviewed_lessons(
            &self,
            student_id: AccountId,
            lesson_ids: &BTreeSet<LessonCompletionId>,
        ) -> Result<BTreeSet<LessonCompletionId>, Unavailable> {
            self.asked
                .lock()
                .unwrap()
                .push((student_id, lesson_ids.clone()));
            Ok(self
                .reviewed
                .iter()
                .filter(|(student, id)| *student == student_id && lesson_ids.contains(id))
                .map(|(_, id)| *id)
                .collect())
        }
    }

    type UseCase = ListMyLessonCompletions<
        InMemoryLessonCompletionRepository,
        InMemoryLiveLessonSessionRepository,
        FixedBookings,
        InMemoryVideoRepository,
        RecordingCoaches,
        RecordingReviews,
    >;

    /// `COACH` の枠を生徒 `STUDENT` が予約し（メニュー「フォア改善」の写し）、通話して終えたセッション。
    fn live_lesson(session: u128, booking: u128) -> (LiveLessonSession, Booking) {
        let mut slot = Slot::publish(
            SlotId::from_u128(booking),
            COACH,
            Sport::Tennis,
            at(10),
            at(11),
            SlotCapacity::new(1).unwrap(),
        )
        .unwrap();
        let mut booking = Booking::reserve(
            BookingId::from_u128(booking),
            STUDENT,
            false,
            &mut slot,
            &[],
            at(0),
        )
        .unwrap()
        .with_restored_lesson_menu(Some(BookingLessonMenu::restore(
            LessonMenuId::from_u128(21),
            name("フォア改善"),
            Some(LessonDuration::from_minutes(60).unwrap()),
        )));
        booking.confirm(STUDENT, &mut slot, at(0)).unwrap();
        let mut session = LiveLessonSession::start(
            SessionId::from_u128(session),
            COACH,
            &booking,
            &[],
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
        (session, booking)
    }

    /// `student` が `coach` に送り、返却された動画（メニューは `menu`）。
    fn reviewed_video(
        id: u128,
        student: AccountId,
        coach: AccountId,
        menu: Option<VideoLessonMenu>,
    ) -> Video {
        let mut video = Video::restore(
            VideoId::from_u128(id),
            student,
            coach,
            Sport::Tennis,
            at(0),
            None,
            menu,
        )
        .unwrap();
        video
            .return_review(
                VideoReviewId::from_u128(id),
                coach,
                ReviewComment::new("よい").unwrap(),
                true,
                at(1),
            )
            .unwrap();
        video
    }

    /// 生徒 `STUDENT` が受けたレッスン（完了した時刻の古い順に、メニューのない動画 61 を 12 時、通話 50 を 14 時、
    /// 「フォーム添削」の動画 60 を 14 時（通話と同じ時刻）、表示名の引けないコーチの動画 62 を 16 時に完了）と、
    /// 別の生徒の動画 63 の完了を記録した保存先と、生徒 `STUDENT` の完了記録（記録した順）。
    async fn fixture(
        coaches: RecordingCoaches,
        reviews: impl FnOnce(&[LessonCompletion]) -> RecordingReviews,
    ) -> (UseCase, Vec<LessonCompletion>) {
        let (session, booking) = live_lesson(50, 30);
        let with_menu = reviewed_video(
            60,
            STUDENT,
            COACH,
            Some(VideoLessonMenu::restore(
                LessonMenuId::from_u128(23),
                name("フォーム添削"),
            )),
        );
        let without_menu = reviewed_video(61, STUDENT, COACH, None);
        let other_coach = reviewed_video(62, STUDENT, OTHER_COACH, None);
        let others = reviewed_video(63, OTHER_STUDENT, COACH, None);

        let completions = InMemoryLessonCompletionRepository::default();
        let mut recorded = Vec::new();
        let id = completions.next_id();
        recorded.push(
            completions
                .record(STUDENT, |history| {
                    without_menu.complete(id, STUDENT, history, at(12))
                })
                .await
                .unwrap(),
        );
        let id = completions.next_id();
        recorded.push(
            completions
                .record(STUDENT, |history| {
                    session.complete_for(id, COACH, STUDENT, history, at(14))
                })
                .await
                .unwrap(),
        );
        for (video, hour) in [(&with_menu, 14), (&other_coach, 16)] {
            let id = completions.next_id();
            recorded.push(
                completions
                    .record(STUDENT, |history| {
                        video.complete(id, STUDENT, history, at(hour))
                    })
                    .await
                    .unwrap(),
            );
        }
        let id = completions.next_id();
        completions
            .record(OTHER_STUDENT, |history| {
                others.complete(id, OTHER_STUDENT, history, at(20))
            })
            .await
            .unwrap();

        let reviews = reviews(&recorded);
        let use_case = ListMyLessonCompletions::new(
            completions,
            InMemoryLiveLessonSessionRepository::with_sessions([session]),
            FixedBookings(vec![booking]),
            InMemoryVideoRepository::with_videos([with_menu, without_menu, other_coach, others]),
            coaches,
            reviews,
        );
        (use_case, recorded)
    }

    #[tokio::test]
    async fn lists_own_live_and_async_lessons_latest_completion_first() {
        let coaches = RecordingCoaches::default();
        let (use_case, recorded) = fixture(coaches.clone(), |_| RecordingReviews::default()).await;

        let list = use_case.execute(STUDENT).await.unwrap();

        // 16 時の動画 62 → 14 時の通話 50 と動画 60（同じ時刻は完了記録の ID の順）→ 12 時の動画 61
        let (live, with_menu) = (&recorded[1], &recorded[2]);
        let mut same_time = [live.id(), with_menu.id()];
        same_time.sort();
        let expected = [
            recorded[3].id(),
            same_time[0],
            same_time[1],
            recorded[0].id(),
        ];
        assert_eq!(
            list.completions
                .iter()
                .map(|item| item.completion.id())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            list.completions
                .iter()
                .all(|item| item.completion.student_id() == STUDENT),
            "別の生徒の完了したレッスンは出ない"
        );

        let item = |id: LessonCompletionId| {
            list.completions
                .iter()
                .find(|item| item.completion.id() == id)
                .unwrap()
        };
        // 通話は予約の、動画は動画のメニューの写し。メニューのない元は None
        assert_eq!(
            item(live.id()).completion.source(),
            LessonSource::Live(SessionId::from_u128(50))
        );
        assert_eq!(
            item(live.id()).lesson_menu,
            Some(LessonCompletionMenu::new(
                LessonMenuId::from_u128(21),
                name("フォア改善")
            ))
        );
        assert_eq!(
            item(with_menu.id()).lesson_menu,
            Some(LessonCompletionMenu::new(
                LessonMenuId::from_u128(23),
                name("フォーム添削")
            ))
        );
        assert_eq!(item(recorded[0].id()).lesson_menu, None);

        assert_eq!(
            list.coaches,
            BTreeMap::from([(COACH, DisplayName::new("たなか").unwrap())]),
            "表示名が引けないコーチ（OTHER_COACH）は含まない"
        );
        assert_eq!(
            *coaches.asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "一覧のコーチを重ねずに 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn marks_only_the_lessons_the_student_reviewed() {
        let asked = AskedReviews::default();
        let (use_case, recorded) = fixture(RecordingCoaches::default(), |recorded| {
            RecordingReviews {
                // 生徒は通話 50 にだけ評価を付けた。動画 61 の評価は別の生徒のもの（印にしない）
                reviewed: vec![
                    (STUDENT, recorded[1].id()),
                    (OTHER_STUDENT, recorded[0].id()),
                ],
                asked: asked.clone(),
            }
        })
        .await;

        let list = use_case.execute(STUDENT).await.unwrap();

        let reviewed: Vec<LessonCompletionId> = list
            .completions
            .iter()
            .filter(|item| item.reviewed)
            .map(|item| item.completion.id())
            .collect();
        assert_eq!(reviewed, [recorded[1].id()]);
        assert_eq!(
            *asked.lock().unwrap(),
            [(
                STUDENT,
                recorded
                    .iter()
                    .map(LessonCompletion::id)
                    .collect::<BTreeSet<_>>()
            )],
            "その生徒の完了したレッスンを 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn a_student_without_lessons_gets_an_empty_list_without_asking_anything() {
        let coaches = RecordingCoaches::default();
        let asked = AskedReviews::default();
        let (use_case, _) = fixture(coaches.clone(), |_| RecordingReviews {
            reviewed: Vec::new(),
            asked: asked.clone(),
        })
        .await;

        let list = use_case.execute(AccountId::from_u128(99)).await.unwrap();

        assert_eq!(list, MyLessonCompletionList::default());
        assert!(coaches.asked.lock().unwrap().is_empty());
        assert!(asked.lock().unwrap().is_empty());
    }
}
