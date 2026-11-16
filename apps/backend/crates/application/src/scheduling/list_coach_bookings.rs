use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

use domain::{
    identity::AccountId,
    live_lesson::{LiveLessonSession, SessionStatus},
    scheduling::{Booking, BookingId, BookingStatus, Slot},
};
use time::Date;

use super::{
    BookingRepository, CoachBookingList, ListBookingsError, StudentDisplayNameLookup, calendar_day,
};
use crate::{Unavailable, clock::Clock, live_lesson::LiveLessonSessionRepository};

/// コーチの自分の予約の一覧のユースケース（admin-web の `listMyCoachBookings`。コーチだけが呼ぶ）。
///
/// コーチ `coach_id` のスロットへの予約を、キャンセル済みも含めて予約されたスロットと一緒に読み、スロットの開始時刻の
/// 古い順に並べる（同じ開始時刻は予約の作成の新しい順、それも同じなら予約 ID の順）。日付の `from`・`to` をどちらも
/// 省けば期間で絞らない。どちらかがあれば、本部が見る予約の一覧（[`super::ListBookings`]）と同じ規則（[`calendar_day::period`]）で
/// 期間にし、スロットの開始がその期間に入る予約だけに絞る。一覧に出る生徒の表示名は profile に問い合わせて
/// （[`StudentDisplayNameLookup`]）添える。予約が結び付いた通話中のセッションを live-lesson の保存先から読んで添える
/// （[`LiveLessonSessionRepository::find_by_booking`]。グループレッスンでは結び付いた予約のどれからでも引ける）。
/// セッションを読むのは、確定済みで枠の開始時刻を過ぎた予約だけ（セッションは確定した予約の枠の開始からしか始められない）。
/// 終了したセッション（本部が終わらせたものを含む）は添えない。読むだけで何も変えない。
pub struct ListCoachBookings<B, L, D, C> {
    bookings: B,
    sessions: L,
    students: D,
    clock: C,
}

/// コーチの自分の予約の一覧の条件。どれも省ける。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListCoachBookingsQuery {
    /// 期間の始まりの日付（日本時間。含む）
    pub from: Option<Date>,
    /// 期間の終わりの日付（日本時間。含む）
    pub to: Option<Date>,
}

impl<B, L, D, C> ListCoachBookings<B, L, D, C>
where
    B: BookingRepository,
    L: LiveLessonSessionRepository,
    D: StudentDisplayNameLookup,
    C: Clock,
{
    pub fn new(bookings: B, sessions: L, students: D, clock: C) -> Self {
        Self {
            bookings,
            sessions,
            students,
            clock,
        }
    }

    pub async fn execute(
        &self,
        coach_id: AccountId,
        query: ListCoachBookingsQuery,
    ) -> Result<CoachBookingList, ListBookingsError> {
        let now = self.clock.now();
        let dates = if query.from.is_none() && query.to.is_none() {
            None
        } else {
            Some(calendar_day::period(query.from, query.to, now)?)
        };
        let mut items = match dates {
            None => self.bookings.bookings_of_coach(coach_id).await?,
            Some((_, _, period)) => {
                let mut items = self
                    .bookings
                    .bookings_of_coach_starting_between(
                        coach_id,
                        period.starts_at(),
                        period.ends_at(),
                    )
                    .await?;
                items.retain(|(_, slot)| period.includes(slot.starts_at()));
                items
            }
        };
        items.sort_by_key(|(booking, slot)| {
            (
                slot.starts_at(),
                Reverse(booking.created_at()),
                booking.id(),
            )
        });
        let student_ids: BTreeSet<AccountId> = items
            .iter()
            .map(|(booking, _)| booking.student_id())
            .collect();
        let students = if student_ids.is_empty() {
            Default::default()
        } else {
            self.students
                .find_student_display_names(&student_ids)
                .await?
        };
        let sessions = self.sessions_in_progress(&items, now).await?;
        Ok(CoachBookingList {
            from: dates.map(|(from, _, _)| from),
            to: dates.map(|(_, to, _)| to),
            items,
            students,
            sessions,
        })
    }

    /// `items` の予約のうち、確定済みで枠の開始時刻（`now` まで）を過ぎた予約が結び付いた通話中のセッション（予約 ID
    /// ごと）。グループレッスンのセッションは 1 回読めば、結び付いた予約のうち一覧のものすべてに入れる（読み直さない）。
    async fn sessions_in_progress(
        &self,
        items: &[(Booking, Slot)],
        now: SystemTime,
    ) -> Result<BTreeMap<BookingId, LiveLessonSession>, Unavailable> {
        let started: BTreeSet<BookingId> = items
            .iter()
            .filter(|(booking, slot)| {
                booking.status() == BookingStatus::Confirmed && slot.starts_at() <= now
            })
            .map(|(booking, _)| booking.id())
            .collect();
        let mut asked = BTreeSet::new();
        let mut sessions = BTreeMap::new();
        for booking_id in &started {
            if !asked.insert(*booking_id) {
                continue;
            }
            let Some(session) = self.sessions.find_by_booking(*booking_id).await? else {
                continue;
            };
            let linked: Vec<BookingId> = session
                .bookings()
                .iter()
                .map(|linked| linked.booking_id())
                .chain([*booking_id])
                .filter(|linked| started.contains(linked))
                .collect();
            asked.extend(linked.iter().copied());
            if session.status() != SessionStatus::InProgress {
                continue;
            }
            for linked in linked {
                sessions.insert(linked, session.clone());
            }
        }
        Ok(sessions)
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
        catalog::Sport,
        live_lesson::{SessionBooking, SessionId},
        profile::DisplayName,
        scheduling::{SchedulingError, SlotCapacity, SlotId, SlotSeats, SlotStatus},
    };
    use time::Month;

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{
            FixedClock, InMemoryBookingRepository, InMemoryLiveLessonSessionRepository,
            InMemorySlotRepository,
        },
        scheduling::BookingRepository,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn date(day: u8) -> Date {
        Date::from_calendar_date(2026, Month::November, day).unwrap()
    }

    /// 2026 年 11 月 `day` 日の日本時間 0 時から `hour` 時間後（の UNIX 紀元からの時間数）
    fn hour_of(day: u8, hour: u64) -> u64 {
        calendar_day::start(date(day))
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 3600
            + hour
    }

    /// 今は 2026 年 11 月 2 日の日本時間 12 時
    fn clock() -> FixedClock {
        FixedClock(at(hour_of(2, 12)))
    }

    fn slot(id: u128, coach: AccountId, hour: u64) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            at(hour),
            at(hour + 1),
            SlotStatus::Open,
            domain::scheduling::SlotSeats::default(),
        )
        .unwrap()
    }

    fn name() -> DisplayName {
        DisplayName::new("あおき").unwrap()
    }

    /// `STUDENT` の表示名だけを知っていて（`OTHER_STUDENT` は引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl StudentDisplayNameLookup for RecordingLookup {
        async fn find_student_display_names(
            &self,
            student_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, DisplayName>, Unavailable> {
            self.asked.lock().unwrap().push(student_ids.clone());
            Ok(student_ids
                .iter()
                .filter(|id| **id == STUDENT)
                .map(|id| (*id, name()))
                .collect())
        }
    }

    async fn book(
        bookings: &InMemoryBookingRepository,
        student: AccountId,
        slot: u128,
        created_hour: u64,
    ) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(
                    bookings.next_id(),
                    student,
                    false,
                    slot,
                    existing,
                    at(created_hour),
                )
            })
            .await
            .unwrap()
    }

    fn ids(list: &CoachBookingList) -> Vec<(u128, BookingStatus)> {
        list.items
            .iter()
            .map(|(booking, slot)| (slot.id().as_u128(), booking.status()))
            .collect()
    }

    #[tokio::test]
    async fn lists_bookings_of_own_slots_including_cancelled_ones_oldest_slot_first() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, 40),
            slot(2, COACH, 20),
            slot(3, COACH, 30),
            slot(4, OTHER_COACH, 10),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, STUDENT, 1, 0).await;
        let cancelled = book(&bookings, OTHER_STUDENT, 3, 1).await;
        bookings
            .update(cancelled.id(), |booking, slot| {
                booking.cancel(OTHER_STUDENT, slot, at(2), true).map(|_| ())
            })
            .await
            .unwrap();
        // キャンセルされた枠を取り直した予約は、キャンセル済みより前に出る
        book(&bookings, STUDENT, 3, 3).await;
        book(&bookings, OTHER_STUDENT, 2, 4).await;
        // 別のコーチの枠への予約は出ない
        book(&bookings, STUDENT, 4, 5).await;

        let lookup = RecordingLookup::default();
        let list = ListCoachBookings::new(
            bookings,
            InMemoryLiveLessonSessionRepository::default(),
            lookup.clone(),
            clock(),
        )
        .execute(COACH, ListCoachBookingsQuery::default())
        .await
        .unwrap();
        assert_eq!(
            ids(&list),
            [
                (2, BookingStatus::Pending),
                (3, BookingStatus::Pending),
                (3, BookingStatus::Cancelled),
                (1, BookingStatus::Pending),
            ]
        );
        assert!(
            list.items
                .iter()
                .all(|(booking, slot)| booking.coach_id() == COACH
                    && slot.coach_id() == COACH
                    && booking.slot_id() == slot.id())
        );
        assert_eq!(
            list.students,
            BTreeMap::from([(STUDENT, name())]),
            "表示名が引けない生徒（OTHER_STUDENT）は含まない"
        );
        assert_eq!(
            *lookup.asked.lock().unwrap(),
            [BTreeSet::from([STUDENT, OTHER_STUDENT])],
            "一覧の生徒を重ねずに 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn coach_without_bookings_gets_an_empty_list_without_asking_for_display_names() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH, 10),
            slot(2, OTHER_COACH, 10),
        ]));
        book(&bookings, STUDENT, 2, 0).await;
        let lookup = RecordingLookup::default();
        let list = ListCoachBookings::new(
            bookings,
            InMemoryLiveLessonSessionRepository::default(),
            lookup.clone(),
            clock(),
        )
        .execute(COACH, ListCoachBookingsQuery::default())
        .await
        .unwrap();
        assert!(list.items.is_empty());
        assert!(list.students.is_empty());
        assert!(lookup.asked.lock().unwrap().is_empty());
    }

    /// `COACH` の枠を 11 月 1 日 23 時・2 日 0 時・8 日 23 時・9 日 0 時・20 日 10 時（日本時間）に置き、どれも予約する。
    /// 別のコーチの 3 日の枠への予約も 1 件ある。
    async fn bookings_around_next_week() -> InMemoryBookingRepository {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH, hour_of(1, 23)),
            slot(2, COACH, hour_of(2, 0)),
            slot(3, COACH, hour_of(8, 23)),
            slot(4, COACH, hour_of(9, 0)),
            slot(5, COACH, hour_of(20, 10)),
            slot(6, OTHER_COACH, hour_of(3, 10)),
        ]));
        for id in 1..=6 {
            book(&bookings, STUDENT, id, 0).await;
        }
        bookings
    }

    async fn slot_ids(query: ListCoachBookingsQuery) -> Result<Vec<u128>, ListBookingsError> {
        let list = ListCoachBookings::new(
            bookings_around_next_week().await,
            InMemoryLiveLessonSessionRepository::default(),
            RecordingLookup::default(),
            clock(),
        )
        .execute(COACH, query)
        .await?;
        Ok(list
            .items
            .iter()
            .map(|(_, slot)| slot.id().as_u128())
            .collect())
    }

    #[tokio::test]
    async fn without_dates_lists_every_booking_as_before() {
        let list = ListCoachBookings::new(
            bookings_around_next_week().await,
            InMemoryLiveLessonSessionRepository::default(),
            RecordingLookup::default(),
            clock(),
        )
        .execute(COACH, ListCoachBookingsQuery::default())
        .await
        .unwrap();
        assert_eq!(
            list.items
                .iter()
                .map(|(_, slot)| slot.id().as_u128())
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        assert_eq!((list.from, list.to), (None, None), "期間で絞らない");
    }

    #[tokio::test]
    async fn dates_narrow_to_bookings_whose_slot_starts_within_them_in_japan_time() {
        // 1 日 23 時と 9 日 0 時（日本時間）の枠は入らない。別のコーチの枠も出ない
        assert_eq!(slot_ids(query_from_to(2, 8)).await.unwrap(), [2, 3]);
        let list = ListCoachBookings::new(
            bookings_around_next_week().await,
            InMemoryLiveLessonSessionRepository::default(),
            RecordingLookup::default(),
            clock(),
        )
        .execute(COACH, query_from_to(2, 8))
        .await
        .unwrap();
        assert_eq!((list.from, list.to), (Some(date(2)), Some(date(8))));
    }

    fn query_from_to(from: u8, to: u8) -> ListCoachBookingsQuery {
        ListCoachBookingsQuery {
            from: Some(date(from)),
            to: Some(date(to)),
        }
    }

    #[tokio::test]
    async fn omitted_from_or_to_follow_the_same_rule_as_list_bookings() {
        // to だけなら from は今日（2 日）
        let to_only = ListCoachBookingsQuery {
            from: None,
            to: Some(date(9)),
        };
        assert_eq!(slot_ids(to_only).await.unwrap(), [2, 3, 4]);
        // from だけなら to は from から 7 日目（9 日から 15 日）
        let from_only = ListCoachBookingsQuery {
            from: Some(date(9)),
            to: None,
        };
        let list = ListCoachBookings::new(
            bookings_around_next_week().await,
            InMemoryLiveLessonSessionRepository::default(),
            RecordingLookup::default(),
            clock(),
        )
        .execute(COACH, from_only)
        .await
        .unwrap();
        assert_eq!((list.from, list.to), (Some(date(9)), Some(date(15))));
        assert_eq!(
            list.items
                .iter()
                .map(|(_, slot)| slot.id().as_u128())
                .collect::<Vec<_>>(),
            [4]
        );
    }

    #[tokio::test]
    async fn rejects_dates_that_break_the_period_rule() {
        assert!(matches!(
            slot_ids(query_from_to(8, 2)).await,
            Err(ListBookingsError::Invalid(
                SchedulingError::BookingPeriodEndsBeforeItStarts
            ))
        ));
        assert!(matches!(
            slot_ids(ListCoachBookingsQuery {
                from: None,
                to: Some(date(1)),
            })
            .await,
            Err(ListBookingsError::Invalid(
                SchedulingError::BookingPeriodEndsBeforeItStarts
            ))
        ));
        let too_long = ListCoachBookingsQuery {
            from: Some(date(1)),
            to: Some(Date::from_calendar_date(2026, Month::December, 2).unwrap()),
        };
        assert!(matches!(
            slot_ids(too_long).await,
            Err(ListBookingsError::Invalid(
                SchedulingError::BookingPeriodTooLong { max_days: 31 }
            ))
        ));
    }

    const THIRD_STUDENT: AccountId = AccountId::from_u128(3);
    const HEADQUARTERS: AccountId = AccountId::from_u128(9);

    /// 定員 `capacity` の `COACH` の枠 `id`（`hour` 時に始まる）
    fn group_slot(id: u128, hour: u64, capacity: i64) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            at(hour),
            at(hour + 1),
            SlotStatus::Open,
            SlotSeats::vacant(SlotCapacity::new(capacity).unwrap()),
        )
        .unwrap()
    }

    async fn confirmed(
        bookings: &InMemoryBookingRepository,
        student: AccountId,
        slot: u128,
    ) -> Booking {
        let booking = book(bookings, student, slot, 0).await;
        bookings
            .update(booking.id(), |booking, slot| {
                booking.confirm(student, slot, at(1))
            })
            .await
            .unwrap()
    }

    /// `first` で開始し、`others` も結び付いた、`hour` 時に始まった通話中のセッション `id`
    fn session(id: u128, first: &Booking, others: &[&Booking], hour: u64) -> LiveLessonSession {
        LiveLessonSession::restore(
            SessionId::from_u128(id),
            first.id(),
            first.student_id(),
            COACH,
            others
                .iter()
                .map(|other| SessionBooking::new(other.id(), other.student_id()))
                .collect(),
            at(hour),
            None,
        )
        .unwrap()
    }

    /// 問い合わせた予約を記録する、セッションの保存先
    #[derive(Clone)]
    struct RecordingSessions {
        inner: InMemoryLiveLessonSessionRepository,
        asked: Arc<Mutex<Vec<BookingId>>>,
    }

    impl RecordingSessions {
        fn with_sessions(sessions: impl IntoIterator<Item = LiveLessonSession>) -> Self {
            Self {
                inner: InMemoryLiveLessonSessionRepository::with_sessions(sessions),
                asked: Arc::default(),
            }
        }
    }

    impl LiveLessonSessionRepository for RecordingSessions {
        fn next_id(&self) -> SessionId {
            self.inner.next_id()
        }

        async fn find(
            &self,
            session_id: SessionId,
        ) -> Result<Option<LiveLessonSession>, Unavailable> {
            self.inner.find(session_id).await
        }

        async fn find_by_booking(
            &self,
            booking_id: BookingId,
        ) -> Result<Option<LiveLessonSession>, Unavailable> {
            self.asked.lock().unwrap().push(booking_id);
            self.inner.find_by_booking(booking_id).await
        }

        async fn find_in_progress_started_at_or_before(
            &self,
            started_at_or_before: SystemTime,
            coach_id: Option<AccountId>,
            student_id: Option<AccountId>,
        ) -> Result<Vec<LiveLessonSession>, Unavailable> {
            self.inner
                .find_in_progress_started_at_or_before(started_at_or_before, coach_id, student_id)
                .await
        }

        async fn find_started_between(
            &self,
            starts_at: SystemTime,
            ends_at: SystemTime,
            coach_id: Option<AccountId>,
            student_id: Option<AccountId>,
        ) -> Result<Vec<LiveLessonSession>, Unavailable> {
            self.inner
                .find_started_between(starts_at, ends_at, coach_id, student_id)
                .await
        }

        async fn insert(
            &self,
            session: LiveLessonSession,
        ) -> Result<(), crate::live_lesson::InsertSessionError> {
            self.inner.insert(session).await
        }

        async fn update<F>(
            &self,
            session_id: SessionId,
            update: F,
        ) -> Result<LiveLessonSession, crate::live_lesson::UpdateSessionError>
        where
            F: FnOnce(&mut LiveLessonSession) -> Result<(), domain::DomainError> + Send,
        {
            self.inner.update(session_id, update).await
        }
    }

    async fn list_with(
        bookings: InMemoryBookingRepository,
        sessions: RecordingSessions,
    ) -> CoachBookingList {
        ListCoachBookings::new(bookings, sessions, RecordingLookup::default(), clock())
            .execute(COACH, ListCoachBookingsQuery::default())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn bookings_in_a_call_carry_the_session_in_progress_and_others_do_not() {
        let now = hour_of(2, 12);
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH, now - 1),
            slot(2, COACH, now - 3),
            slot(3, COACH, now - 2),
        ]));
        let in_call = confirmed(&bookings, STUDENT, 1).await;
        let ended = confirmed(&bookings, STUDENT, 2).await;
        let without_session = confirmed(&bookings, OTHER_STUDENT, 3).await;
        let mut ended_by_headquarters = session(11, &ended, &[], now - 3);
        ended_by_headquarters
            .end_by_headquarters(HEADQUARTERS, at(now - 2))
            .unwrap();
        let sessions = RecordingSessions::with_sessions([
            session(10, &in_call, &[], now - 1),
            ended_by_headquarters,
        ]);

        let list = list_with(bookings, sessions).await;
        assert_eq!(list.sessions.len(), 1, "通話中のセッションの予約だけに載る");
        let session = &list.sessions[&in_call.id()];
        assert_eq!(session.id(), SessionId::from_u128(10));
        assert_eq!(session.status(), SessionStatus::InProgress);
        assert_eq!(session.started_at(), at(now - 1));
        assert!(
            !list.sessions.contains_key(&ended.id()),
            "本部が終わらせたセッションは終了扱いで載らない"
        );
        assert!(!list.sessions.contains_key(&without_session.id()));
    }

    #[tokio::test]
    async fn every_booking_linked_to_a_group_session_carries_it_with_a_single_lookup() {
        let now = hour_of(2, 12);
        let bookings =
            InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([group_slot(
                1,
                now - 1,
                3,
            )]));
        let first = confirmed(&bookings, STUDENT, 1).await;
        let second = confirmed(&bookings, OTHER_STUDENT, 1).await;
        let third = confirmed(&bookings, THIRD_STUDENT, 1).await;
        // 2 人目の予約で開始したセッション（3 人目の予約は結び付いた予約の 2 番目以降）
        let sessions =
            RecordingSessions::with_sessions([session(10, &second, &[&first, &third], now - 1)]);

        let list = list_with(bookings, sessions.clone()).await;
        for booking in [&first, &second, &third] {
            assert_eq!(
                list.sessions.get(&booking.id()).map(LiveLessonSession::id),
                Some(SessionId::from_u128(10)),
                "結び付いた予約のどれにも同じセッションが載る"
            );
        }
        assert_eq!(
            sessions.asked.lock().unwrap().len(),
            1,
            "結び付いた予約のセッションは読み直さない"
        );
    }

    #[tokio::test]
    async fn sessions_are_not_looked_up_for_unconfirmed_or_not_yet_started_bookings() {
        let now = hour_of(2, 12);
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([
            slot(1, COACH, now - 1),
            slot(2, COACH, now + 1),
            slot(3, COACH, now),
        ]));
        book(&bookings, STUDENT, 1, 0).await;
        confirmed(&bookings, STUDENT, 2).await;
        let starting_now = confirmed(&bookings, OTHER_STUDENT, 3).await;
        let sessions = RecordingSessions::with_sessions([]);

        let list = list_with(bookings, sessions.clone()).await;
        assert!(list.sessions.is_empty());
        assert_eq!(
            *sessions.asked.lock().unwrap(),
            [starting_now.id()],
            "確定済みで枠の開始時刻（ちょうどを含む）を過ぎた予約だけ問い合わせる"
        );
    }
}
