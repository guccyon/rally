use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet},
};

use domain::{identity::AccountId, scheduling::CancellationPolicy};

use super::{BookingRepository, CoachIntroductionLookup, MyBookingList, QuotaGate};
use crate::{Unavailable, clock::Clock};

/// 本人の予約の一覧のユースケース（student-app の `listMyBookings`。生徒だけが呼ぶ）。
///
/// 生徒 `student_id` の予約を、キャンセル済みも含めて予約したスロットと一緒に読み、スロットの開始時刻の新しい順に
/// 並べる（同じ開始時刻は予約の作成の新しい順、それも同じなら予約 ID の順）。一覧に出るコーチの紹介は profile に
/// 問い合わせて（[`CoachIntroductionLookup`]）添える。キャンセルできる予約には、今（[`Clock`]）キャンセルしたら
/// 回数を使うかの見込みを domain の [`CancellationPolicy::preview`] で添える（規則はそこだけ。生徒が通話の枠を
/// 持つかは entitlement に問い合わせる [`QuotaGate::has_live_lesson_quota`]）。読むだけで何も変えない。
pub struct ListMyBookings<B, I, G, C> {
    bookings: B,
    coaches: I,
    quota: G,
    clock: C,
}

impl<B, I, G, C> ListMyBookings<B, I, G, C>
where
    B: BookingRepository,
    I: CoachIntroductionLookup,
    G: QuotaGate,
    C: Clock,
{
    pub fn new(bookings: B, coaches: I, quota: G, clock: C) -> Self {
        Self {
            bookings,
            coaches,
            quota,
            clock,
        }
    }

    pub async fn execute(&self, student_id: AccountId) -> Result<MyBookingList, Unavailable> {
        let now = self.clock.now();
        let mut items = self.bookings.bookings_of_student(student_id).await?;
        items.sort_by_key(|(booking, slot)| {
            (
                Reverse(slot.starts_at()),
                Reverse(booking.created_at()),
                booking.id(),
            )
        });
        let coach_ids: BTreeSet<AccountId> =
            items.iter().map(|(_, slot)| slot.coach_id()).collect();
        let coaches = if coach_ids.is_empty() {
            Default::default()
        } else {
            self.coaches.find_coach_introductions(&coach_ids).await?
        };
        let cancellations = if items
            .iter()
            .any(|(booking, _)| booking.status().is_active())
        {
            let has_live_lesson_quota = self.quota.has_live_lesson_quota(student_id).await?;
            items
                .iter()
                .filter_map(|(booking, _)| {
                    CancellationPolicy::preview(booking, now, has_live_lesson_quota)
                        .map(|preview| (booking.id(), preview))
                })
                .collect()
        } else {
            BTreeMap::new()
        };
        Ok(MyBookingList {
            items,
            coaches,
            cancellations,
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
        catalog::Sport,
        entitlement::{
            Entitlement, EntitlementId, LiveLessonQuota, SubscriptionId, SubscriptionPlan,
            YearMonth,
        },
        profile::{CoachBio, DisplayName},
        scheduling::{Booking, BookingStatus, CancellationPreview, Slot, SlotId, SlotStatus},
    };

    use super::*;
    use crate::{
        entitlement::EntitlementQuotaGate,
        in_memory::{
            FixedClock, InMemoryBookingRepository, InMemoryEntitlementRepository,
            InMemorySlotRepository,
        },
        scheduling::CoachIntroduction,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
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

    fn introduction() -> CoachIntroduction {
        CoachIntroduction {
            display_name: DisplayName::new("たなか").unwrap(),
            bio: CoachBio::default(),
        }
    }

    /// `COACH` の紹介だけを知っていて（`OTHER_COACH` は紹介が引けない）、問い合わせた ID を記録する。
    #[derive(Clone, Default)]
    struct RecordingLookup {
        asked: Arc<Mutex<Vec<BTreeSet<AccountId>>>>,
    }

    impl CoachIntroductionLookup for RecordingLookup {
        async fn find_coach_introductions(
            &self,
            coach_ids: &BTreeSet<AccountId>,
        ) -> Result<BTreeMap<AccountId, CoachIntroduction>, Unavailable> {
            self.asked.lock().unwrap().push(coach_ids.clone());
            Ok(coach_ids
                .iter()
                .filter(|id| **id == COACH)
                .map(|id| (*id, introduction()))
                .collect())
        }
    }

    type UseCase = ListMyBookings<
        InMemoryBookingRepository,
        RecordingLookup,
        EntitlementQuotaGate<InMemoryEntitlementRepository>,
        FixedClock,
    >;

    /// 受講権のない生徒の一覧を、`now_hour` 時に読むユースケース。
    fn use_case(
        bookings: InMemoryBookingRepository,
        lookup: RecordingLookup,
        now_hour: u64,
    ) -> UseCase {
        use_case_with(bookings, lookup, [], now_hour)
    }

    fn use_case_with(
        bookings: InMemoryBookingRepository,
        lookup: RecordingLookup,
        entitlements: impl IntoIterator<Item = Entitlement>,
        now_hour: u64,
    ) -> UseCase {
        ListMyBookings::new(
            bookings,
            lookup,
            EntitlementQuotaGate::new(InMemoryEntitlementRepository::with_entitlements(
                entitlements,
            )),
            FixedClock(at(now_hour)),
        )
    }

    /// `STUDENT` の月額（standard）の受講権。
    fn monthly() -> Entitlement {
        Entitlement::monthly(
            EntitlementId::from_u128(100),
            STUDENT,
            Sport::Tennis,
            SubscriptionId::from_u128(200),
            LiveLessonQuota::restore(
                SubscriptionPlan::Standard,
                YearMonth::new(1970, 1).unwrap(),
                [],
            )
            .unwrap(),
        )
        .unwrap()
    }

    /// `STUDENT` のワンショット（ゴルフ）の受講権。
    fn one_shot() -> Entitlement {
        Entitlement::one_shot(EntitlementId::from_u128(101), STUDENT, Sport::Golf, 1).unwrap()
    }

    async fn book(
        bookings: &InMemoryBookingRepository,
        student: AccountId,
        slot: u128,
        created_hour: u64,
    ) -> Booking {
        book_as(bookings, student, slot, created_hour, false).await
    }

    async fn book_as(
        bookings: &InMemoryBookingRepository,
        student: AccountId,
        slot: u128,
        created_hour: u64,
        is_trial: bool,
    ) -> Booking {
        bookings
            .reserve(SlotId::from_u128(slot), student, |slot, existing| {
                Booking::reserve(
                    bookings.next_id(),
                    student,
                    is_trial,
                    slot,
                    existing,
                    at(created_hour),
                )
            })
            .await
            .unwrap()
    }

    async fn cancel(bookings: &InMemoryBookingRepository, booking: &Booking, hour: u64) {
        bookings
            .update(booking.id(), |booking, slot| {
                booking.cancel(STUDENT, slot, at(hour)).map(|_| ())
            })
            .await
            .unwrap();
    }

    fn preview(consumes_quota: bool, free_cancel_until: Option<u64>) -> CancellationPreview {
        CancellationPreview {
            consumes_quota,
            free_cancel_until: free_cancel_until.map(at),
        }
    }

    fn ids(list: &MyBookingList) -> Vec<(u128, BookingStatus)> {
        list.items
            .iter()
            .map(|(booking, slot)| (slot.id().as_u128(), booking.status()))
            .collect()
    }

    #[tokio::test]
    async fn lists_own_bookings_including_cancelled_ones_newest_slot_first() {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, 10),
            slot(2, OTHER_COACH, 30),
            slot(3, COACH, 20),
            slot(4, COACH, 40),
        ]);
        let bookings = InMemoryBookingRepository::new(slots);
        book(&bookings, STUDENT, 1, 0).await;
        let cancelled = book(&bookings, STUDENT, 3, 1).await;
        cancel(&bookings, &cancelled, 2).await;
        // キャンセルした枠を取り直した予約は、キャンセル済みより前に出る
        book(&bookings, STUDENT, 3, 3).await;
        book(&bookings, STUDENT, 2, 4).await;
        // 別の生徒の予約は出ない
        book(&bookings, OTHER_STUDENT, 4, 5).await;

        let lookup = RecordingLookup::default();
        let list = use_case(bookings, lookup.clone(), 5)
            .execute(STUDENT)
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
        assert!(list.items.iter().all(
            |(booking, slot)| booking.student_id() == STUDENT && booking.slot_id() == slot.id()
        ));
        assert_eq!(
            list.coaches,
            BTreeMap::from([(COACH, introduction())]),
            "紹介が引けないコーチ（OTHER_COACH）は含まない"
        );
        assert_eq!(
            *lookup.asked.lock().unwrap(),
            [BTreeSet::from([COACH, OTHER_COACH])],
            "一覧のコーチを重ねずに 1 回で問い合わせる"
        );
    }

    #[tokio::test]
    async fn student_without_bookings_gets_an_empty_list_without_asking_for_introductions() {
        let bookings = InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(
            1, COACH, 10,
        )]));
        book(&bookings, OTHER_STUDENT, 1, 0).await;
        let lookup = RecordingLookup::default();
        let list = use_case(bookings, lookup.clone(), 5)
            .execute(STUDENT)
            .await
            .unwrap();
        assert!(list.items.is_empty());
        assert!(list.coaches.is_empty());
        assert!(lookup.asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancellation_preview_switches_at_24_hours_before_start() {
        // 開始は 100 時。境目は 76 時（24 時間前）
        let slots = InMemorySlotRepository::with_slots([slot(1, COACH, 100)]);
        let bookings = InMemoryBookingRepository::new(slots);
        let booking = book(&bookings, STUDENT, 1, 0).await;
        for (now_hour, consumes_quota) in [(75, false), (76, true), (99, true), (101, true)] {
            let list = use_case_with(
                bookings.clone(),
                RecordingLookup::default(),
                [monthly()],
                now_hour,
            )
            .execute(STUDENT)
            .await
            .unwrap();
            assert_eq!(
                list.cancellations,
                BTreeMap::from([(booking.id(), preview(consumes_quota, Some(76)))]),
                "{now_hour} 時"
            );
        }
    }

    #[tokio::test]
    async fn preview_matches_the_decision_of_cancelling_at_the_same_time() {
        let slots = InMemorySlotRepository::with_slots([slot(1, COACH, 100)]);
        let bookings = InMemoryBookingRepository::new(slots);
        let booking = book(&bookings, STUDENT, 1, 0).await;
        for now_hour in [75, 76] {
            let list = use_case_with(
                bookings.clone(),
                RecordingLookup::default(),
                [monthly()],
                now_hour,
            )
            .execute(STUDENT)
            .await
            .unwrap();
            assert_eq!(
                list.cancellations[&booking.id()].consumes_quota,
                CancellationPolicy::decide(&booking, at(now_hour)).consumes_quota
            );
        }
    }

    #[tokio::test]
    async fn cancelled_bookings_have_no_preview() {
        let slots = InMemorySlotRepository::with_slots([slot(1, COACH, 100), slot(2, COACH, 200)]);
        let bookings = InMemoryBookingRepository::new(slots);
        let cancelled = book(&bookings, STUDENT, 1, 0).await;
        cancel(&bookings, &cancelled, 1).await;
        let active = book(&bookings, STUDENT, 2, 2).await;

        let list = use_case_with(bookings, RecordingLookup::default(), [monthly()], 80)
            .execute(STUDENT)
            .await
            .unwrap();
        assert_eq!(list.items.len(), 2);
        assert_eq!(
            list.cancellations,
            BTreeMap::from([(active.id(), preview(false, Some(176)))]),
            "キャンセル済みの予約には見込みを付けない"
        );
    }

    #[tokio::test]
    async fn bookings_of_students_without_the_live_lesson_quota_never_consume() {
        // 開始は 100 時で、読むのは境目（76 時）を過ぎた 90 時
        for (entitlements, is_trial) in [
            // 月額契約の前の体験予約
            (vec![], true),
            // ワンショットの受講権だけ
            (vec![one_shot()], false),
        ] {
            let bookings =
                InMemoryBookingRepository::new(InMemorySlotRepository::with_slots([slot(
                    1, COACH, 100,
                )]));
            let booking = book_as(&bookings, STUDENT, 1, 0, is_trial).await;
            let list = use_case_with(bookings, RecordingLookup::default(), entitlements, 90)
                .execute(STUDENT)
                .await
                .unwrap();
            assert_eq!(
                list.cancellations,
                BTreeMap::from([(booking.id(), preview(false, None))]),
                "体験予約 {is_trial}"
            );
        }
    }

    #[tokio::test]
    async fn trial_booking_after_subscribing_follows_the_policy() {
        let slots = InMemorySlotRepository::with_slots([slot(1, COACH, 100)]);
        let bookings = InMemoryBookingRepository::new(slots);
        let trial = book_as(&bookings, STUDENT, 1, 0, true).await;
        let list = use_case_with(bookings, RecordingLookup::default(), [monthly()], 90)
            .execute(STUDENT)
            .await
            .unwrap();
        assert_eq!(
            list.cancellations,
            BTreeMap::from([(trial.id(), preview(true, Some(76)))]),
            "月額契約の後の体験予約は他の予約と同じに数える（docs/adr/0005 の 6）"
        );
    }
}
