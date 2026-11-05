use std::{collections::BTreeSet, time::SystemTime};

use super::{
    Booking, BookingPeriod, CancellationDecision, CoachLeaveBooking, CoachLeaveId, CoachLeaveNote,
    CoachLeaveRequest, CoachLeaveTargets, SchedulingError, Slot, SlotStatus,
};
use crate::identity::AccountId;

/// コーチのお休み（本部がコーチの期間の予約と枠をまとめて片づけた記録）。
///
/// お休みにする（[`CoachLeave::start`]）と、期間の有効な予約（[`CoachLeaveTargets`]）をすべて本部のキャンセル
/// （[`Booking::cancel_by_headquarters`]。回数を消費しない）にし、期間の終了でない枠をすべて取り下げる（[`Slot::withdraw`]）。
/// 全部か何もしないかで、途中で断れば予約も枠も変えない。記録は期間・生徒への一言・操作した本部のアカウント・時刻・キャンセル
/// した予約の写し（[`CoachLeaveBooking`]）・取り下げた枠の数を持つ。取り消し（予約を元に戻す）はない。
///
/// 同じコーチの期間の重なるお休みは 2 つ持てない（同じ期間の 2 回目の操作もこれで断る）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachLeave {
    id: CoachLeaveId,
    coach_id: AccountId,
    period: BookingPeriod,
    note: CoachLeaveNote,
    created_by: AccountId,
    created_at: SystemTime,
    cancelled_bookings: Vec<CoachLeaveBooking>,
    withdrawn_slot_count: u32,
}

impl CoachLeave {
    /// `request` のとおりに、時刻 `now` にコーチをお休みにする。`slots` と `bookings` は少なくとも、コーチの開始が期間に入る
    /// 枠とそれらの枠への予約（保存先が読み取りから保存までを直列にして渡す）、`existing` は少なくともそのコーチのお休みの記録。
    /// キャンセルした予約と取り下げた枠は `slots`・`bookings` の中で変える。返す判定はキャンセルした予約ごと（記録の
    /// [`Self::cancelled_bookings`] と同じ並び）。
    ///
    /// 次のときは断り、`slots` も `bookings` も変えない。
    ///
    /// - 期間の始まりが `now` より後でない（始まりの日が今日以前）: [`SchedulingError::CoachLeaveStartsTooSoon`]（今日のうちに
    ///   始まる・通話中の予約を巻き込まない）
    /// - 同じコーチの期間の重なるお休みがある: [`SchedulingError::CoachLeaveOverlaps`]
    /// - 期間の有効な予約の ID が、本部が確かめたもの（`request.expected_booking_ids`）と違う（確かめたあとに予約が入った・
    ///   キャンセルされた）: [`SchedulingError::CoachLeavePreviewOutdated`]
    /// - 予約のキャンセル・枠の取り下げが不変条件を破る（保存先が渡した値が合わない）: その [`SchedulingError`]
    pub fn start(
        id: CoachLeaveId,
        request: &CoachLeaveRequest,
        now: SystemTime,
        slots: &mut [Slot],
        bookings: &mut [Booking],
        existing: &[CoachLeave],
    ) -> Result<(Self, Vec<CancellationDecision>), SchedulingError> {
        let period = request.period;
        Self::ensure_starts_after(period, now)?;
        if existing
            .iter()
            .any(|leave| leave.coach_id == request.coach_id && leave.overlaps(&period))
        {
            return Err(SchedulingError::CoachLeaveOverlaps);
        }
        let targets = CoachLeaveTargets::of(request.coach_id, period, slots, bookings);
        if targets.booking_ids() != request.expected_booking_ids {
            return Err(SchedulingError::CoachLeavePreviewOutdated);
        }

        // 失敗したときに書きかけを残さないよう、写しに適用してから書き戻す
        let mut next_slots: Vec<Slot> = slots
            .iter()
            .filter(|slot| slot.coach_id() == request.coach_id && period.includes(slot.starts_at()))
            .cloned()
            .collect();
        let mut next_bookings = Vec::with_capacity(targets.bookings().len());
        let mut cancelled = Vec::with_capacity(targets.bookings().len());
        let mut decisions = Vec::with_capacity(targets.bookings().len());
        for (booking, _) in targets.bookings() {
            let mut booking = booking.clone();
            let slot = next_slots
                .iter_mut()
                .find(|slot| slot.id() == booking.slot_id())
                .ok_or(SchedulingError::BookingOfAnotherSlot)?;
            decisions.push(booking.cancel_by_headquarters(slot, now)?);
            cancelled.push(CoachLeaveBooking::new(
                booking.id(),
                booking.student_id(),
                slot.starts_at(),
                slot.ends_at(),
            ));
            next_bookings.push(booking);
        }
        let mut withdrawn: u32 = 0;
        for slot in next_slots
            .iter_mut()
            .filter(|slot| slot.status() != SlotStatus::Closed)
        {
            slot.withdraw(request.coach_id, now)?;
            withdrawn += 1;
        }
        write_back(slots, next_slots, Slot::id);
        write_back(bookings, next_bookings, Booking::id);

        Ok((
            Self {
                id,
                coach_id: request.coach_id,
                period,
                note: request.note.clone(),
                created_by: request.requested_by,
                created_at: now,
                cancelled_bookings: cancelled,
                withdrawn_slot_count: withdrawn,
            },
            decisions,
        ))
    }

    /// お休みの期間 `period` が `now` より後に始まるか（始まりの日が明日以降か）。でなければ
    /// [`SchedulingError::CoachLeaveStartsTooSoon`]。期間の予約と枠の確かめと、お休みにする操作（[`Self::start`]）が共に使う。
    pub fn ensure_starts_after(
        period: BookingPeriod,
        now: SystemTime,
    ) -> Result<(), SchedulingError> {
        if period.starts_at() <= now {
            return Err(SchedulingError::CoachLeaveStartsTooSoon);
        }
        Ok(())
    }

    /// 保存先から読んだお休みの記録を組み立てる（保存先の実装用。不変条件はお休みにした時点で確かめたので、ここでは
    /// 確かめない）。
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: CoachLeaveId,
        coach_id: AccountId,
        period: BookingPeriod,
        note: CoachLeaveNote,
        created_by: AccountId,
        created_at: SystemTime,
        cancelled_bookings: Vec<CoachLeaveBooking>,
        withdrawn_slot_count: u32,
    ) -> Self {
        Self {
            id,
            coach_id,
            period,
            note,
            created_by,
            created_at,
            cancelled_bookings,
            withdrawn_slot_count,
        }
    }

    pub const fn id(&self) -> CoachLeaveId {
        self.id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// お休みの期間。
    pub const fn period(&self) -> BookingPeriod {
        self.period
    }

    /// 生徒への一言（書かなければ空）。
    pub const fn note(&self) -> &CoachLeaveNote {
        &self.note
    }

    /// 操作した本部のアカウント。
    pub const fn created_by(&self) -> AccountId {
        self.created_by
    }

    /// お休みにした時刻（予約をキャンセルし、枠を取り下げた時刻）。
    pub const fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// キャンセルした予約の写し（枠の開始の早い順）。
    pub fn cancelled_bookings(&self) -> &[CoachLeaveBooking] {
        &self.cancelled_bookings
    }

    /// 取り下げた枠の数（予約のあった枠を含む）。
    pub const fn withdrawn_slot_count(&self) -> u32 {
        self.withdrawn_slot_count
    }

    /// お知らせを送った生徒の人数（キャンセルした予約の生徒。同じ生徒は 1 人と数える）。
    pub fn notified_student_count(&self) -> usize {
        self.cancelled_bookings
            .iter()
            .map(CoachLeaveBooking::student_id)
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// 期間 `period` と重なるか（どちらも始まりを含み終わりを含まない。終わりの日の翌日に始まる期間は重ならない）。
    pub fn overlaps(&self, period: &BookingPeriod) -> bool {
        self.period.starts_at() < period.ends_at() && period.starts_at() < self.period.ends_at()
    }
}

/// `items` のうち `next` と同じ ID のものを `next` で置き換える。
fn write_back<T, K: PartialEq>(items: &mut [T], next: Vec<T>, key: impl Fn(&T) -> K) {
    for next in next {
        if let Some(item) = items.iter_mut().find(|item| key(item) == key(&next)) {
            *item = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        catalog::Sport,
        scheduling::{BookingCanceller, BookingId, BookingStatus, SlotCapacity, SlotId},
    };

    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);
    const HEADQUARTERS: AccountId = AccountId::from_u128(9);
    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const DAY: Duration = BookingPeriod::DAY;
    const HOUR: Duration = Duration::from_secs(3600);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;

    /// `NOW` から `days` 日後の `hour` 時。
    fn at(days: u32, hour: u32) -> SystemTime {
        NOW + DAY * days + HOUR * hour
    }

    /// `from` 日目から `to` 日目まで（両端を含む）の期間。
    fn period(from: u32, to: u32) -> BookingPeriod {
        BookingPeriod::new(at(from, 0), at(to + 1, 0)).unwrap()
    }

    fn slot(id: u128, coach: AccountId, days: u32, hour: u32, capacity: i64) -> Slot {
        Slot::publish(
            SlotId::from_u128(id),
            coach,
            Sport::Tennis,
            at(days, hour),
            at(days, hour + 1),
            SlotCapacity::new(capacity).unwrap(),
        )
        .unwrap()
    }

    fn reserve(id: u128, student: AccountId, slot: &mut Slot, confirm: bool) -> Booking {
        let mut booking =
            Booking::reserve(BookingId::from_u128(id), student, false, slot, &[], NOW).unwrap();
        if confirm {
            booking.confirm(student, slot, NOW).unwrap();
        }
        booking
    }

    /// 2〜4 日目のお休みの対象になる枠と予約。確定済みの予約のある枠（2 日目）・確定待ちの予約のある枠（3 日目）・
    /// 予約のない枠（3 日目の夜）・グループの枠に 2 人（4 日目）と、対象の外（期間の外の 5 日目と、ほかのコーチの 3 日目）。
    fn calendar() -> (Vec<Slot>, Vec<Booking>) {
        let mut booked = slot(10, COACH, 2, 19, 1);
        let mut pending = slot(11, COACH, 3, 10, 1);
        let open = slot(12, COACH, 3, 20, 1);
        let mut group = slot(13, COACH, 4, 10, 3);
        let mut outside = slot(14, COACH, 5, 10, 1);
        let mut other_coach = slot(15, OTHER_COACH, 3, 10, 1);
        let bookings = vec![
            reserve(1, STUDENT, &mut booked, true),
            reserve(2, OTHER_STUDENT, &mut pending, false),
            reserve(3, STUDENT, &mut group, true),
            reserve(4, OTHER_STUDENT, &mut group, false),
            reserve(5, STUDENT, &mut outside, true),
            reserve(6, STUDENT, &mut other_coach, true),
        ];
        (
            vec![booked, pending, open, group, outside, other_coach],
            bookings,
        )
    }

    fn request(period: BookingPeriod, expected: &[u128]) -> CoachLeaveRequest {
        CoachLeaveRequest {
            coach_id: COACH,
            period,
            note: CoachLeaveNote::new("来週からは予約できます。").unwrap(),
            requested_by: HEADQUARTERS,
            expected_booking_ids: expected
                .iter()
                .map(|id| BookingId::from_u128(*id))
                .collect(),
        }
    }

    fn status_of(bookings: &[Booking], id: u128) -> BookingStatus {
        bookings
            .iter()
            .find(|b| b.id() == BookingId::from_u128(id))
            .unwrap()
            .status()
    }

    fn slot_status(slots: &[Slot], id: u128) -> SlotStatus {
        slots
            .iter()
            .find(|s| s.id() == SlotId::from_u128(id))
            .unwrap()
            .status()
    }

    #[test]
    fn the_targets_are_the_active_bookings_and_open_slots_starting_in_the_period() {
        let (slots, bookings) = calendar();
        let targets = CoachLeaveTargets::of(COACH, period(2, 4), &slots, &bookings);

        let ids: Vec<_> = targets
            .bookings()
            .iter()
            .map(|(b, _)| b.id().as_u128())
            .collect();
        assert_eq!(
            ids,
            vec![1, 2, 3, 4],
            "開始の早い順。グループの枠は予約ごと"
        );
        let open: Vec<_> = targets
            .open_slots()
            .iter()
            .map(|s| s.id().as_u128())
            .collect();
        assert_eq!(open, vec![12], "予約のない公開中の枠だけ");
        assert_eq!(targets.slot_count(), 4, "予約のある枠も取り下げる");
        assert_eq!(targets.student_count(), 2, "同じ生徒は 1 人");
        assert!(!targets.is_empty());

        let empty = CoachLeaveTargets::of(COACH, period(20, 21), &slots, &bookings);
        assert!(empty.is_empty());
        assert_eq!((empty.slot_count(), empty.student_count()), (0, 0));
    }

    #[test]
    fn starting_a_leave_cancels_the_bookings_and_withdraws_the_slots_in_the_period() {
        let (mut slots, mut bookings) = calendar();
        let now = at(1, 12);

        let (leave, decisions) = CoachLeave::start(
            CoachLeaveId::from_u128(100),
            &request(period(2, 4), &[1, 2, 3, 4]),
            now,
            &mut slots,
            &mut bookings,
            &[],
        )
        .unwrap();

        for id in [1, 2, 3, 4] {
            assert_eq!(status_of(&bookings, id), BookingStatus::Cancelled);
            let booking = bookings
                .iter()
                .find(|b| b.id() == BookingId::from_u128(id))
                .unwrap();
            assert_eq!(booking.cancelled_by(), Some(BookingCanceller::Headquarters));
            assert_eq!(booking.cancelled_at(), Some(now));
        }
        assert!(
            decisions.iter().all(|decision| !decision.consumes_quota),
            "本部のキャンセルは回数を消費しない"
        );
        assert_eq!(decisions.len(), 4);
        for id in [10, 11, 12, 13] {
            assert_eq!(slot_status(&slots, id), SlotStatus::Closed);
        }
        // 期間の外とほかのコーチはそのまま
        assert_eq!(status_of(&bookings, 5), BookingStatus::Confirmed);
        assert_eq!(status_of(&bookings, 6), BookingStatus::Confirmed);
        assert_eq!(slot_status(&slots, 14), SlotStatus::Booked);
        assert_eq!(slot_status(&slots, 15), SlotStatus::Booked);

        assert_eq!(leave.id(), CoachLeaveId::from_u128(100));
        assert_eq!(leave.coach_id(), COACH);
        assert_eq!(leave.period(), period(2, 4));
        assert_eq!(leave.note().as_str(), "来週からは予約できます。");
        assert_eq!(
            (leave.created_by(), leave.created_at()),
            (HEADQUARTERS, now)
        );
        assert_eq!(leave.withdrawn_slot_count(), 4);
        assert_eq!(leave.notified_student_count(), 2);
        let copied: Vec<_> = leave
            .cancelled_bookings()
            .iter()
            .map(|b| {
                (
                    b.booking_id().as_u128(),
                    b.student_id(),
                    b.starts_at(),
                    b.ends_at(),
                )
            })
            .collect();
        assert_eq!(
            copied,
            vec![
                (1, STUDENT, at(2, 19), at(2, 20)),
                (2, OTHER_STUDENT, at(3, 10), at(3, 11)),
                (3, STUDENT, at(4, 10), at(4, 11)),
                (4, OTHER_STUDENT, at(4, 10), at(4, 11)),
            ]
        );
    }

    #[test]
    fn a_leave_without_bookings_or_slots_is_recorded_with_zero_counts() {
        let (mut slots, mut bookings) = calendar();
        let (leave, decisions) = CoachLeave::start(
            CoachLeaveId::from_u128(100),
            &request(period(20, 21), &[]),
            at(1, 12),
            &mut slots,
            &mut bookings,
            &[],
        )
        .unwrap();
        assert!(decisions.is_empty());
        assert_eq!(leave.withdrawn_slot_count(), 0);
        assert_eq!(leave.notified_student_count(), 0);
        assert_eq!((slots, bookings), calendar());
    }

    #[test]
    fn rejections_change_nothing() {
        let existing = CoachLeave::restore(
            CoachLeaveId::from_u128(99),
            COACH,
            period(4, 6),
            CoachLeaveNote::default(),
            HEADQUARTERS,
            NOW,
            Vec::new(),
            0,
        );
        for (request, now, existing, expected) in [
            (
                request(period(2, 4), &[1, 2, 3, 4]),
                at(2, 0),
                vec![],
                SchedulingError::CoachLeaveStartsTooSoon,
            ),
            (
                request(period(2, 4), &[1, 2, 3, 4]),
                at(1, 12),
                vec![existing.clone()],
                SchedulingError::CoachLeaveOverlaps,
            ),
            (
                request(period(2, 4), &[1, 2, 3]),
                at(1, 12),
                vec![],
                SchedulingError::CoachLeavePreviewOutdated,
            ),
            (
                request(period(2, 4), &[1, 2, 3, 4, 5]),
                at(1, 12),
                vec![],
                SchedulingError::CoachLeavePreviewOutdated,
            ),
        ] {
            let (mut slots, mut bookings) = calendar();
            assert_eq!(
                CoachLeave::start(
                    CoachLeaveId::from_u128(100),
                    &request,
                    now,
                    &mut slots,
                    &mut bookings,
                    &existing,
                )
                .map(|_| ()),
                Err(expected)
            );
            assert_eq!((slots, bookings), calendar());
        }
    }

    #[test]
    fn leaves_overlap_only_when_their_days_overlap() {
        let leave = CoachLeave::restore(
            CoachLeaveId::from_u128(99),
            COACH,
            period(4, 6),
            CoachLeaveNote::default(),
            HEADQUARTERS,
            NOW,
            Vec::new(),
            0,
        );
        assert!(leave.overlaps(&period(6, 8)));
        assert!(leave.overlaps(&period(1, 4)));
        assert!(leave.overlaps(&period(5, 5)));
        assert!(
            !leave.overlaps(&period(7, 9)),
            "終わりの日の翌日からは重ならない"
        );
        assert!(!leave.overlaps(&period(1, 3)));
    }

    #[test]
    fn another_coach_leave_does_not_overlap() {
        let (mut slots, mut bookings) = calendar();
        let mut other = request(period(2, 4), &[6]);
        other.coach_id = OTHER_COACH;
        let existing = CoachLeave::restore(
            CoachLeaveId::from_u128(99),
            COACH,
            period(2, 4),
            CoachLeaveNote::default(),
            HEADQUARTERS,
            NOW,
            Vec::new(),
            0,
        );
        let (leave, _) = CoachLeave::start(
            CoachLeaveId::from_u128(100),
            &other,
            at(1, 12),
            &mut slots,
            &mut bookings,
            &[existing],
        )
        .unwrap();
        assert_eq!(leave.withdrawn_slot_count(), 1);
        assert_eq!(status_of(&bookings, 6), BookingStatus::Cancelled);
        assert_eq!(status_of(&bookings, 1), BookingStatus::Confirmed);
    }
}
