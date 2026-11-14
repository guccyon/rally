use std::time::SystemTime;

use super::{
    BookingCancelReason, BookingCanceller, BookingId, BookingLessonMenu, BookingStatus,
    BookingStatusChange, CancellationDecision, CancellationPolicy, CoachLeave, SchedulingError,
    Slot, SlotId,
};
use crate::{
    catalog::{LessonMenu, LessonMenuId},
    identity::{Account, AccountId},
    profile::CoachSportFilter,
};

/// 予約（glossary の booking）。生徒によるスロットの確保で、体験予約（`is_trial`）もこれ。
///
/// 作成した直後は [`BookingStatus::Pending`] でスロットの席を 1 つ押さえ、確定（[`Booking::confirm`]）すると
/// その席が確定になる（確定した予約が定員に達したスロットは [`SlotStatus::Booked`](super::SlotStatus::Booked)）。キャンセル
/// （[`Booking::cancel`]）すると [`BookingStatus::Cancelled`] になり、席が空いてスロットは公開中に戻る。コーチと開始時刻は予約したスロットのもの。
/// 予約しても受講権は消費しない（product-context の用語の注意）。
///
/// 予約した時点でスロットに結びついていたレッスンメニュー（[`BookingLessonMenu`]）の写しを任意で持つ
/// （[`Booking::with_lesson_menu`]）。予約した後にメニューが直されても写しは変わらない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Booking {
    id: BookingId,
    student_id: AccountId,
    slot_id: SlotId,
    coach_id: AccountId,
    is_trial: bool,
    status: BookingStatus,
    starts_at: SystemTime,
    created_at: SystemTime,
    confirmed_at: Option<SystemTime>,
    cancelled_at: Option<SystemTime>,
    cancelled_by: Option<BookingCanceller>,
    cancel_reason: Option<BookingCancelReason>,
    cancellation_decision: Option<CancellationDecision>,
    lesson_menu: Option<BookingLessonMenu>,
}

impl Booking {
    /// 生徒 `student_id` が `slot` を予約する（状態は [`BookingStatus::Pending`]）。`slot` の席を 1 つ押さえる
    /// （[`Slot::hold`]）。
    ///
    /// `existing` には少なくとも、`slot` の予約と `student_id` の生徒の予約を含める（それ以外の予約は
    /// 判定に影響しない）。次の不変条件を破れば [`SchedulingError`] を返し、`slot` は変えない。
    ///
    /// - 公開中（[`SlotStatus::Open`](super::SlotStatus::Open)）のスロットにしか予約できない（[`SchedulingError::SlotNotOpen`]）
    /// - 有効な予約（未確定か確定）はスロットの定員まで（[`SchedulingError::SlotAlreadyBooked`]）
    /// - 同じ生徒が同じスロットに有効な予約を 2 つは持てない（[`SchedulingError::SlotAlreadyBookedByStudent`]）
    /// - 体験予約はその生徒の初回の予約だけ。キャンセルされていない予約が 1 件でもあれば
    ///   [`SchedulingError::TrialAlreadyUsed`]（完了したレッスンの有無は見ない。それも合わせた判定は
    ///   [`Booking::is_trial_available`]）
    pub fn reserve(
        id: BookingId,
        student_id: AccountId,
        is_trial: bool,
        slot: &mut Slot,
        existing: &[Booking],
        now: SystemTime,
    ) -> Result<Self, SchedulingError> {
        slot.ensure_seat_available()?;
        let active = || existing.iter().filter(|b| b.status.is_active());
        if active().any(|b| b.slot_id == slot.id() && b.student_id == student_id) {
            return Err(SchedulingError::SlotAlreadyBookedByStudent);
        }
        if is_trial && !Self::is_trial_available(student_id, existing, false) {
            return Err(SchedulingError::TrialAlreadyUsed);
        }
        slot.hold()?;
        Ok(Self {
            id,
            student_id,
            slot_id: slot.id(),
            coach_id: slot.coach_id(),
            is_trial,
            status: BookingStatus::Pending,
            starts_at: slot.starts_at(),
            created_at: now,
            confirmed_at: None,
            cancelled_at: None,
            cancelled_by: None,
            cancel_reason: None,
            cancellation_decision: None,
            lesson_menu: None,
        })
    }

    /// 生徒 `student_id` が今体験予約できるか（体験をまだ使っていないか）。`bookings` にその生徒のキャンセルされていない
    /// 予約が 1 件もなく、完了したレッスン（lesson_completion。同期・非同期のどちらでも）もない
    /// （`has_completed_lesson` が false）ときだけ true。
    ///
    /// 受け終えた体験予約をあとでキャンセルしても、完了したレッスンが残るので false のまま。受けていない体験予約を
    /// キャンセルしただけなら true。`bookings` の別の生徒の予約は判定に影響しない。
    pub fn is_trial_available<'a>(
        student_id: AccountId,
        bookings: impl IntoIterator<Item = &'a Booking>,
        has_completed_lesson: bool,
    ) -> bool {
        !has_completed_lesson
            && !bookings
                .into_iter()
                .any(|b| b.status.is_active() && b.student_id == student_id)
    }

    /// 作ったばかりの予約（[`Booking::reserve`] の結果）に、予約したスロット `slot` に結びついたレッスンメニュー
    /// `menu`（catalog から引いた今のメニュー。引けなければ `None`）の写しを残す。
    ///
    /// 写すのは、`slot` がこの予約のスロットで、`menu` がそのスロットに結びついたメニュー（同じ ID）で、公開中の
    /// ときだけ。それ以外（スロットにメニューがない・メニューが引けない・公開中でない）はメニューなしの予約にする
    /// （予約は断らない）。予約した後のメニューの変更は写しに映さないので、呼ぶのは予約の作成のときだけ。
    pub fn with_lesson_menu(mut self, slot: &Slot, menu: Option<&LessonMenu>) -> Self {
        self.lesson_menu = menu
            .filter(|menu| {
                slot.id() == self.slot_id
                    && slot.lesson_menu_id() == Some(menu.id())
                    && menu.is_published()
            })
            .map(BookingLessonMenu::copy_of);
        self
    }

    /// 保存先から読んだ予約に、予約した時点のレッスンメニューの写しを戻す（保存先の実装用。写したときに
    /// [`Booking::with_lesson_menu`] で確かめているので、ここでは確かめない）。
    pub fn with_restored_lesson_menu(mut self, lesson_menu: Option<BookingLessonMenu>) -> Self {
        self.lesson_menu = lesson_menu;
        self
    }

    /// 保存先から読んだ予約を組み立てる（保存先の実装用）。コーチと開始時刻は予約したスロット `slot` のもの。
    ///
    /// 確定・キャンセルの時刻は `status` に合わせて保存先が持つ（DB の `bookings_status_timestamps`）。
    /// 不変条件は作成・確定・キャンセルの時点で確かめたので、ここでは確かめない。キャンセル済みの予約のキャンセルした人は
    /// 予約した生徒とし、本部がキャンセルした予約は [`Booking::with_restored_canceller`] で戻す。
    #[allow(clippy::too_many_arguments)]
    pub const fn restore(
        id: BookingId,
        student_id: AccountId,
        slot: &Slot,
        is_trial: bool,
        status: BookingStatus,
        created_at: SystemTime,
        confirmed_at: Option<SystemTime>,
        cancelled_at: Option<SystemTime>,
    ) -> Self {
        Self {
            id,
            student_id,
            slot_id: slot.id(),
            coach_id: slot.coach_id(),
            is_trial,
            status,
            starts_at: slot.starts_at(),
            created_at,
            confirmed_at,
            cancelled_at,
            cancelled_by: match status {
                BookingStatus::Cancelled => Some(BookingCanceller::Student),
                BookingStatus::Pending | BookingStatus::Confirmed => None,
            },
            cancel_reason: None,
            cancellation_decision: None,
            lesson_menu: None,
        }
    }

    /// 保存先から読んだキャンセル済みの予約に、キャンセルした人 `canceller` を戻す（保存先の実装用。保存先がキャンセルした人を
    /// 持たない行（本部のキャンセルができる前にキャンセルした予約）は `None` で、予約した生徒のまま）。キャンセルしていない
    /// 予約には何もしない。
    pub fn with_restored_canceller(mut self, canceller: Option<BookingCanceller>) -> Self {
        if self.status == BookingStatus::Cancelled
            && let Some(canceller) = canceller
        {
            self.cancelled_by = Some(canceller);
        }
        self
    }

    /// 保存先から読んだキャンセル済みの予約に、キャンセルした理由 `reason` とキャンセル規定の判定 `decision` を戻す（保存先の
    /// 実装用）。保存先が持たない行（理由・判定を残し始める前のキャンセル）は `None` のまま。キャンセルしていない予約には何も
    /// しない。
    pub fn with_restored_cancellation(
        mut self,
        reason: Option<BookingCancelReason>,
        decision: Option<CancellationDecision>,
    ) -> Self {
        if self.status == BookingStatus::Cancelled {
            self.cancel_reason = reason;
            self.cancellation_decision = decision;
        }
        self
    }

    /// お休みの記録 `leave` がこの予約をキャンセルした予約として持っていれば、キャンセルした理由を
    /// [`BookingCancelReason::CoachLeave`] にする（理由を残し始める前にお休みでキャンセルした予約も、お休みの記録との
    /// 結びつきから理由が分かる）。本部がキャンセルした予約でないとき・`leave` がこの予約を持たないときは何もしない。
    pub fn with_restored_coach_leave(mut self, leave: &CoachLeave) -> Self {
        if self.cancelled_by == Some(BookingCanceller::Headquarters)
            && leave
                .cancelled_bookings()
                .iter()
                .any(|booking| booking.booking_id() == self.id)
        {
            self.cancel_reason = Some(BookingCancelReason::CoachLeave);
        }
        self
    }

    /// 予約した生徒 `student_id` が確定する。予約したスロット `slot` の席を確定に移し、確定した予約が定員に
    /// 達したら [`SlotStatus::Booked`](super::SlotStatus::Booked) にする（[`Slot::book`]）。
    ///
    /// 別の生徒なら [`SchedulingError::BookingOfAnotherStudent`]、確定前（[`BookingStatus::Pending`]）で
    /// なければ [`SchedulingError::BookingNotPending`]、スロットが公開中でなければ
    /// [`SchedulingError::SlotNotOpen`]。失敗したときは予約もスロットも変えない。
    pub fn confirm(
        &mut self,
        student_id: AccountId,
        slot: &mut Slot,
        now: SystemTime,
    ) -> Result<(), SchedulingError> {
        if student_id != self.student_id {
            return Err(SchedulingError::BookingOfAnotherStudent);
        }
        if slot.id() != self.slot_id {
            return Err(SchedulingError::BookingOfAnotherSlot);
        }
        if self.status != BookingStatus::Pending {
            return Err(SchedulingError::BookingNotPending {
                status: self.status,
            });
        }
        slot.book()?;
        self.status = BookingStatus::Confirmed;
        self.confirmed_at = Some(now);
        Ok(())
    }

    /// 予約した生徒 `student_id` が `now` にキャンセルし、キャンセル規定の判定を返す。
    ///
    /// 判定は [`CancellationPolicy::decide`] だけが行う（回数の消費の実行は entitlement）。予約したスロット `slot` の
    /// 席を 1 つ空ける。確定済みならスロットを公開中（[`SlotStatus::Open`](super::SlotStatus::Open)）に戻す（[`Slot::release`]）。確定前なら
    /// スロットはもともと公開中で、押さえた席だけを空ける（[`Slot::release_hold`]）。
    /// 別の生徒なら [`SchedulingError::BookingOfAnotherStudent`]、キャンセル済みなら
    /// [`SchedulingError::BookingAlreadyCancelled`]、確定済みなのにスロットに確定した予約が入っていなければ
    /// [`SchedulingError::SlotNotBooked`]。失敗したときは予約もスロットも変えない。
    pub fn cancel(
        &mut self,
        student_id: AccountId,
        slot: &mut Slot,
        now: SystemTime,
    ) -> Result<CancellationDecision, SchedulingError> {
        if student_id != self.student_id {
            return Err(SchedulingError::BookingOfAnotherStudent);
        }
        let decision = CancellationPolicy::decide(self, now);
        self.cancel_as(BookingCanceller::Student, None, decision, slot, now)
    }

    /// 本部が `now` にキャンセルし（コーチのお休み。[`super::CoachLeave::start`] から呼ぶ）、キャンセル規定の判定を返す。
    ///
    /// 判定は [`CancellationPolicy::decide_for_headquarters`] だけが行う（生徒の都合でないので回数を消費しない）。スロットの
    /// 席の空け方と失敗は [`Self::cancel`] と同じ（生徒は問わない）。キャンセルした人は [`BookingCanceller::Headquarters`]、
    /// 理由は [`BookingCancelReason::CoachLeave`]。
    pub fn cancel_by_headquarters(
        &mut self,
        slot: &mut Slot,
        now: SystemTime,
    ) -> Result<CancellationDecision, SchedulingError> {
        let decision = CancellationPolicy::decide_for_headquarters(self);
        self.cancel_as(
            BookingCanceller::Headquarters,
            Some(BookingCancelReason::CoachLeave),
            decision,
            slot,
            now,
        )
    }

    /// `canceller` が `now` に理由 `reason` でキャンセルし、判定 `decision` を返す（理由と判定は予約に残す）。予約した
    /// スロット `slot` の席を 1 つ空ける（確定済みなら [`Slot::release`]、確定前なら [`Slot::release_hold`]）。失敗したときは
    /// 予約もスロットも変えない。
    fn cancel_as(
        &mut self,
        canceller: BookingCanceller,
        reason: Option<BookingCancelReason>,
        decision: CancellationDecision,
        slot: &mut Slot,
        now: SystemTime,
    ) -> Result<CancellationDecision, SchedulingError> {
        if slot.id() != self.slot_id {
            return Err(SchedulingError::BookingOfAnotherSlot);
        }
        match self.status {
            BookingStatus::Cancelled => return Err(SchedulingError::BookingAlreadyCancelled),
            BookingStatus::Pending => slot.release_hold(),
            BookingStatus::Confirmed => slot.release()?,
        }
        self.status = BookingStatus::Cancelled;
        self.cancelled_at = Some(now);
        self.cancelled_by = Some(canceller);
        self.cancel_reason = reason;
        self.cancellation_decision = Some(decision);
        Ok(decision)
    }

    pub const fn id(&self) -> BookingId {
        self.id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn slot_id(&self) -> SlotId {
        self.slot_id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    /// 一覧を絞るコーチの指定に当てはまるか。指定があればその ID が予約された枠のコーチのときだけ、なければどの予約も
    /// 当てはまる。コーチでない ID・ない ID はどの予約にも当てはまらない（一覧は空になる）。本部が見る予約の一覧と
    /// キャンセルされた予約の一覧が共に使う。
    pub fn matches_coach(&self, coach_id: Option<AccountId>) -> bool {
        coach_id.is_none_or(|coach_id| self.coach_id == coach_id)
    }

    /// 一覧を絞る生徒の指定に当てはまるか。指定があればその ID が予約した生徒のときだけ、なければどの予約も当てはまる。
    /// 生徒でない ID・ない ID はどの予約にも当てはまらない（一覧は空になる）。本部が見る予約の一覧とキャンセルされた
    /// 予約の一覧が共に使う。
    pub fn matches_student(&self, student_id: Option<AccountId>) -> bool {
        student_id.is_none_or(|student_id| self.student_id == student_id)
    }

    /// 一覧を絞る競技の指定に当てはまるか。予約した枠のコーチのアカウント `coach`（引けなければ `None`）の提供スポーツで
    /// 決め（判定は [`CoachSportFilter`] の 1 か所。予約枠の需給の [`Slot::matches_coach_sport`] と同じ）、指定がなければ
    /// どの予約も当てはまる。指定があるときは、`coach` が予約した枠のコーチのアカウントで競技を提供するときだけ当てはまる
    /// （アカウントが引けない予約は当てはまらない）。本部が見る予約の一覧とキャンセルされた予約の一覧が共に使う。
    pub fn matches_coach_sport(&self, sport: CoachSportFilter, coach: Option<&Account>) -> bool {
        sport.sport().is_none()
            || coach.is_some_and(|coach| coach.id() == self.coach_id && sport.matches(coach))
    }

    pub const fn is_trial(&self) -> bool {
        self.is_trial
    }

    pub const fn status(&self) -> BookingStatus {
        self.status
    }

    /// 予約したスロットの開始時刻。
    pub const fn starts_at(&self) -> SystemTime {
        self.starts_at
    }

    pub const fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// 確定した時刻。確定するまでは `None`。
    pub const fn confirmed_at(&self) -> Option<SystemTime> {
        self.confirmed_at
    }

    /// キャンセルした時刻。キャンセルするまでは `None`。
    pub const fn cancelled_at(&self) -> Option<SystemTime> {
        self.cancelled_at
    }

    /// キャンセルした人。キャンセルしていなければ `None`。
    ///
    /// 予約した生徒のキャンセル（[`Self::cancel`]）は [`BookingCanceller::Student`]、本部のキャンセル
    /// （[`Self::cancel_by_headquarters`]）は [`BookingCanceller::Headquarters`]。保存先はキャンセルした人を保存して読み戻す
    /// （[`Self::with_restored_canceller`]）。
    pub const fn cancelled_by(&self) -> Option<BookingCanceller> {
        self.cancelled_by
    }

    /// キャンセルした理由。キャンセル済みの予約のうち、理由を残したもの（本部のコーチのお休みでのキャンセル）だけが持つ。
    /// 生徒のキャンセル・キャンセルしていない予約・理由を残し始める前のキャンセルは `None`。
    pub const fn cancel_reason(&self) -> Option<BookingCancelReason> {
        self.cancel_reason
    }

    /// キャンセルしたときのキャンセル規定の判定（[`CancellationDecision`]。回数を消費したか）。キャンセルしていない予約と、
    /// 判定を残し始める前のキャンセルは `None`。
    pub const fn cancellation_decision(&self) -> Option<CancellationDecision> {
        self.cancellation_decision
    }

    /// 一覧を絞るキャンセルした人の指定に当てはまるか。指定があればキャンセルした人（[`Self::cancelled_by`]）がそれと同じ
    /// ときだけ、なければどの予約も当てはまる。キャンセルした人のない予約（キャンセルしていない予約）は、指定があればどの
    /// 指定にも当てはまらない。本部が見るキャンセルされた予約の一覧が使う。
    pub fn matches_canceller(&self, canceller: Option<BookingCanceller>) -> bool {
        canceller.is_none_or(|canceller| self.cancelled_by == Some(canceller))
    }

    /// 予約した時点のレッスンメニューの写し。メニューなしの予約は `None`。
    pub const fn lesson_menu(&self) -> Option<&BookingLessonMenu> {
        self.lesson_menu.as_ref()
    }

    /// 一覧を絞るレッスンメニューの指定に当てはまるか。指定があれば予約した時点のメニューの写し（[`Booking::lesson_menu`]）の
    /// ID がそれと同じときだけ、なければどの予約も当てはまる。メニューの写しのない予約・ないメニューの ID は当てはまらない
    /// （一覧は空になる。コーチの指定 [`Booking::matches_coach`] と同じ扱い）。本部が見る予約の一覧と、キャンセルされた予約の
    /// 一覧が使う。
    pub fn matches_lesson_menu(&self, lesson_menu_id: Option<LessonMenuId>) -> bool {
        lesson_menu_id.is_none_or(|id| {
            self.lesson_menu
                .as_ref()
                .is_some_and(|menu| menu.id() == id)
        })
    }

    /// 状態が変わった記録（確定・キャンセル）を時刻の古い順に返す（同じ時刻は確定、キャンセルの順）。予約が持つ確定・
    /// キャンセルの時刻から組み立て、時刻のない変化（確定せずにキャンセルした予約の確定など）は含めない。作ったばかりの
    /// 予約は空。作った時刻は [`Self::created_at`]。キャンセルの記録だけが、キャンセルした人・理由・判定を持つ。
    pub fn status_changes(&self) -> Vec<BookingStatusChange> {
        let confirmed = self
            .confirmed_at
            .map(|at| BookingStatusChange::new(BookingStatus::Confirmed, at));
        let cancelled = self.cancelled_at.map(|at| {
            BookingStatusChange::cancelled(
                at,
                self.cancelled_by,
                self.cancel_reason,
                self.cancellation_decision,
            )
        });
        let mut changes: Vec<BookingStatusChange> =
            confirmed.into_iter().chain(cancelled).collect();
        changes.sort_by_key(BookingStatusChange::changed_at);
        changes
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        catalog::Sport,
        scheduling::{SlotCapacity, SlotSeats, SlotStatus},
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const THIRD_STUDENT: AccountId = AccountId::from_u128(4);
    const COACH: AccountId = AccountId::from_u128(3);
    const NOW: SystemTime = SystemTime::UNIX_EPOCH;

    fn at(hour: u64) -> SystemTime {
        NOW + Duration::from_secs(hour * 3600)
    }

    fn slot_with_capacity(id: u128, status: SlotStatus, capacity: i64) -> Slot {
        let capacity = SlotCapacity::new(capacity).unwrap();
        // 予約済みの枠は確定した予約で定員まで埋まっている
        let confirmed = if status == SlotStatus::Booked {
            capacity.value().into()
        } else {
            0
        };
        Slot::restore(
            SlotId::from_u128(id),
            COACH,
            Sport::Tennis,
            at(id as u64),
            at(id as u64 + 1),
            status,
            SlotSeats::restore(capacity, 0, confirmed).unwrap(),
        )
        .unwrap()
    }

    fn slot(id: u128, status: SlotStatus) -> Slot {
        slot_with_capacity(id, status, 1)
    }

    fn reserve(
        id: u128,
        student: AccountId,
        is_trial: bool,
        slot: &mut Slot,
        existing: &[Booking],
    ) -> Result<Booking, SchedulingError> {
        Booking::reserve(
            BookingId::from_u128(id),
            student,
            is_trial,
            slot,
            existing,
            NOW,
        )
    }

    fn cancelled(mut booking: Booking) -> Booking {
        booking.status = BookingStatus::Cancelled;
        booking.cancelled_at = Some(NOW);
        booking.cancelled_by = Some(BookingCanceller::Student);
        booking
    }

    #[test]
    fn cancelling_a_confirmed_booking_reopens_the_slot_and_returns_the_decision() {
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        booking.confirm(STUDENT, &mut slot, NOW).unwrap();

        // 開始（10 時）の 1 時間前。判定は CancellationPolicy と同じ
        let decision = booking.cancel(STUDENT, &mut slot, at(9)).unwrap();

        assert_eq!(decision, CancellationPolicy::decide(&booking, at(9)));
        assert!(decision.consumes_quota);
        assert_eq!(booking.status(), BookingStatus::Cancelled);
        assert_eq!(booking.cancelled_at(), Some(at(9)));
        assert_eq!(slot.status(), SlotStatus::Open, "スロットは公開中に戻る");
        assert_eq!(slot.remaining_seats(), 1);
    }

    #[test]
    fn cancelling_a_pending_booking_keeps_the_slot_open_and_frees_the_seat() {
        let mut slot = slot(100, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, true, &mut slot, &[]).unwrap();
        assert_eq!(slot.remaining_seats(), 0);

        // 開始（100 時）の 99 時間前
        let decision = booking.cancel(STUDENT, &mut slot, at(1)).unwrap();

        assert!(!decision.consumes_quota);
        assert_eq!(booking.status(), BookingStatus::Cancelled);
        assert_eq!(booking.cancelled_at(), Some(at(1)));
        assert_eq!(slot.status(), SlotStatus::Open);
        assert_eq!(slot.remaining_seats(), 1);
    }

    #[test]
    fn only_a_cancelled_booking_has_the_student_as_its_canceller() {
        let mut slot = slot(100, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        assert_eq!(booking.cancelled_by(), None, "作ったばかり");
        booking.confirm(STUDENT, &mut slot, at(1)).unwrap();
        assert_eq!(booking.cancelled_by(), None, "確定済み");

        booking.cancel(STUDENT, &mut slot, at(2)).unwrap();
        assert_eq!(booking.cancelled_by(), Some(BookingCanceller::Student));

        // 失敗したキャンセルは変えない
        let mut pending = reserve(2, OTHER_STUDENT, false, &mut slot, &[booking.clone()]).unwrap();
        assert!(pending.cancel(STUDENT, &mut slot, at(3)).is_err());
        assert_eq!(pending.cancelled_by(), None);

        // 保存先から読み戻したキャンセル済みの予約も同じ
        let restored = Booking::restore(
            booking.id(),
            STUDENT,
            &slot,
            false,
            BookingStatus::Cancelled,
            NOW,
            None,
            Some(at(2)),
        );
        assert_eq!(restored.cancelled_by(), Some(BookingCanceller::Student));
    }

    #[test]
    fn a_headquarters_cancel_frees_the_seat_without_consuming_quota() {
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        booking.confirm(STUDENT, &mut slot, NOW).unwrap();
        assert_eq!(slot.status(), SlotStatus::Booked);

        // 開始（10 時）の 1 時間前でも、本部のキャンセルは回数を消費しない
        let decision = booking.cancel_by_headquarters(&mut slot, at(9)).unwrap();

        assert!(!decision.consumes_quota);
        assert_eq!(booking.status(), BookingStatus::Cancelled);
        assert_eq!(booking.cancelled_at(), Some(at(9)));
        assert_eq!(booking.cancelled_by(), Some(BookingCanceller::Headquarters));
        assert_eq!(slot.status(), SlotStatus::Open, "スロットは公開中に戻る");
        assert_eq!(slot.remaining_seats(), 1);

        // 2 回目は断り、何も変えない
        let before = (booking.clone(), slot.clone());
        assert_eq!(
            booking.cancel_by_headquarters(&mut slot, at(9)),
            Err(SchedulingError::BookingAlreadyCancelled)
        );
        assert_eq!((booking, slot), before);
    }

    #[test]
    fn the_restored_canceller_applies_only_to_a_cancelled_booking() {
        let slot = slot(10, SlotStatus::Open);
        let restore = |status, cancelled_at| {
            Booking::restore(
                BookingId::from_u128(1),
                STUDENT,
                &slot,
                false,
                status,
                NOW,
                None,
                cancelled_at,
            )
        };
        let cancelled = restore(BookingStatus::Cancelled, Some(at(2)));
        assert_eq!(
            cancelled
                .clone()
                .with_restored_canceller(Some(BookingCanceller::Headquarters))
                .cancelled_by(),
            Some(BookingCanceller::Headquarters)
        );
        // 保存先がキャンセルした人を持たない行は予約した生徒のまま
        assert_eq!(
            cancelled.with_restored_canceller(None).cancelled_by(),
            Some(BookingCanceller::Student)
        );
        assert_eq!(
            restore(BookingStatus::Pending, None)
                .with_restored_canceller(Some(BookingCanceller::Headquarters))
                .cancelled_by(),
            None
        );
    }

    #[test]
    fn the_cancel_record_keeps_the_canceller_the_reason_and_the_decision() {
        // 生徒のキャンセルは理由を持たず、判定を残す
        let mut slot_a = slot(10, SlotStatus::Open);
        let mut by_student = reserve(1, STUDENT, false, &mut slot_a, &[]).unwrap();
        by_student.confirm(STUDENT, &mut slot_a, at(1)).unwrap();
        by_student.cancel(STUDENT, &mut slot_a, at(9)).unwrap();
        assert_eq!(by_student.cancel_reason(), None);
        let changes = by_student.status_changes();
        assert_eq!(changes.len(), 2);
        let (confirmed, cancelled) = (changes[0], changes[1]);
        assert_eq!(confirmed.status(), BookingStatus::Confirmed);
        assert_eq!(confirmed.cancelled_by(), None, "確定の記録は経緯を持たない");
        assert_eq!(confirmed.cancellation_decision(), None);
        assert_eq!(cancelled.status(), BookingStatus::Cancelled);
        assert_eq!(cancelled.changed_at(), at(9));
        assert_eq!(cancelled.cancelled_by(), Some(BookingCanceller::Student));
        assert_eq!(cancelled.cancel_reason(), None);
        assert_eq!(
            cancelled.cancellation_decision(),
            Some(CancellationDecision {
                consumes_quota: true
            })
        );

        // 本部のキャンセル（コーチのお休み）は理由 coach_leave と、消費しない判定を残す
        let mut slot_b = slot(20, SlotStatus::Open);
        let mut by_headquarters = reserve(2, STUDENT, false, &mut slot_b, &[]).unwrap();
        by_headquarters
            .cancel_by_headquarters(&mut slot_b, at(19))
            .unwrap();
        assert_eq!(
            by_headquarters.cancel_reason(),
            Some(BookingCancelReason::CoachLeave)
        );
        let changes = by_headquarters.status_changes();
        assert_eq!(changes.len(), 1, "確定せずにキャンセルした");
        assert_eq!(
            changes[0].cancelled_by(),
            Some(BookingCanceller::Headquarters)
        );
        assert_eq!(
            changes[0].cancel_reason(),
            Some(BookingCancelReason::CoachLeave)
        );
        assert_eq!(
            changes[0].cancellation_decision(),
            Some(CancellationDecision {
                consumes_quota: false
            })
        );
    }

    #[test]
    fn the_restored_cancellation_applies_only_to_a_cancelled_booking() {
        let slot = slot(10, SlotStatus::Open);
        let restore = |status, cancelled_at| {
            Booking::restore(
                BookingId::from_u128(1),
                STUDENT,
                &slot,
                false,
                status,
                NOW,
                None,
                cancelled_at,
            )
        };
        let decision = Some(CancellationDecision {
            consumes_quota: false,
        });
        let cancelled = restore(BookingStatus::Cancelled, Some(at(2)))
            .with_restored_cancellation(Some(BookingCancelReason::CoachLeave), decision);
        assert_eq!(
            cancelled.cancel_reason(),
            Some(BookingCancelReason::CoachLeave)
        );
        assert_eq!(cancelled.cancellation_decision(), decision);
        // 理由・判定を残し始める前のキャンセルは None のまま
        let before =
            restore(BookingStatus::Cancelled, Some(at(2))).with_restored_cancellation(None, None);
        assert_eq!(before.cancel_reason(), None);
        assert_eq!(before.status_changes()[0].cancellation_decision(), None);
        let pending = restore(BookingStatus::Pending, None)
            .with_restored_cancellation(Some(BookingCancelReason::CoachLeave), decision);
        assert_eq!(pending.cancel_reason(), None);
        assert_eq!(pending.cancellation_decision(), None);
    }

    #[test]
    fn a_booking_in_a_coach_leave_was_cancelled_for_the_coach_leave() {
        use crate::scheduling::{BookingPeriod, CoachLeaveBooking, CoachLeaveId, CoachLeaveNote};

        let slot = slot(10, SlotStatus::Open);
        let restore = |id: u128, status, cancelled_at| {
            Booking::restore(
                BookingId::from_u128(id),
                STUDENT,
                &slot,
                false,
                status,
                NOW,
                None,
                cancelled_at,
            )
        };
        let leave = CoachLeave::restore(
            CoachLeaveId::from_u128(5),
            COACH,
            BookingPeriod::new(at(0), at(24)).unwrap(),
            CoachLeaveNote::new("").unwrap(),
            AccountId::from_u128(9),
            NOW,
            vec![CoachLeaveBooking::new(
                BookingId::from_u128(1),
                STUDENT,
                at(10),
                at(11),
            )],
            1,
        );
        // 理由を残し始める前に本部がキャンセルした予約（理由の列が NULL）
        let in_leave = restore(1, BookingStatus::Cancelled, Some(at(2)))
            .with_restored_canceller(Some(BookingCanceller::Headquarters))
            .with_restored_coach_leave(&leave);
        assert_eq!(
            in_leave.cancel_reason(),
            Some(BookingCancelReason::CoachLeave)
        );
        assert_eq!(in_leave.cancellation_decision(), None, "判定の記録はない");
        // お休みの記録にない予約・生徒のキャンセルは変えない
        let other = restore(2, BookingStatus::Cancelled, Some(at(2)))
            .with_restored_canceller(Some(BookingCanceller::Headquarters))
            .with_restored_coach_leave(&leave);
        assert_eq!(other.cancel_reason(), None);
        let by_student =
            restore(1, BookingStatus::Cancelled, Some(at(2))).with_restored_coach_leave(&leave);
        assert_eq!(by_student.cancel_reason(), None);
    }
    #[test]
    fn cancel_failures_change_nothing() {
        let mut open = slot(10, SlotStatus::Open);
        let pending = reserve(1, STUDENT, false, &mut open, &[]).unwrap();
        let mut booked = open.clone();
        let mut confirmed = pending.clone();
        confirmed.confirm(STUDENT, &mut booked, NOW).unwrap();

        for (mut booking, student, mut slot, expected) in [
            (
                pending.clone(),
                OTHER_STUDENT,
                open.clone(),
                SchedulingError::BookingOfAnotherStudent,
            ),
            (
                pending.clone(),
                STUDENT,
                self::slot(11, SlotStatus::Open),
                SchedulingError::BookingOfAnotherSlot,
            ),
            (
                cancelled(pending.clone()),
                STUDENT,
                open.clone(),
                SchedulingError::BookingAlreadyCancelled,
            ),
            (
                confirmed.clone(),
                STUDENT,
                open.clone(),
                SchedulingError::SlotNotBooked {
                    status: SlotStatus::Open,
                },
            ),
        ] {
            let (before, slot_before) = (booking.clone(), slot.clone());
            assert_eq!(booking.cancel(student, &mut slot, at(2)), Err(expected));
            assert_eq!((booking, slot), (before, slot_before));
        }
    }

    #[test]
    fn reserves_an_open_slot_as_pending() {
        let mut slot = slot(10, SlotStatus::Open);
        let booking = reserve(1, STUDENT, true, &mut slot, &[]).unwrap();
        assert_eq!(booking.status(), BookingStatus::Pending);
        assert_eq!(booking.slot_id(), slot.id());
        assert_eq!(booking.coach_id(), COACH);
        assert_eq!(booking.starts_at(), at(10));
        assert_eq!(booking.created_at(), NOW);
        assert_eq!(booking.confirmed_at(), None);
        assert!(booking.is_trial());
        assert_eq!(slot.seats().pending(), 1, "席を 1 つ押さえる");
        assert_eq!(slot.status(), SlotStatus::Open);
    }

    #[test]
    fn regular_booking_is_not_a_trial() {
        let mut slot = slot(10, SlotStatus::Open);
        let booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        assert!(!booking.is_trial());
    }

    #[test]
    fn trial_stays_a_trial_after_it_is_confirmed_or_cancelled() {
        for cancel in [false, true] {
            let mut slot = slot(10, SlotStatus::Open);
            let mut booking = reserve(1, STUDENT, true, &mut slot, &[]).unwrap();
            if cancel {
                booking.cancel(STUDENT, &mut slot, at(1)).unwrap();
            } else {
                booking.confirm(STUDENT, &mut slot, at(1)).unwrap();
            }
            assert!(booking.is_trial(), "状態が変わっても体験予約のまま");
        }
    }

    #[test]
    fn only_open_slots_can_be_reserved() {
        for status in [SlotStatus::Booked, SlotStatus::Closed] {
            assert_eq!(
                reserve(1, STUDENT, false, &mut slot(10, status), &[]),
                Err(SchedulingError::SlotNotOpen { status })
            );
        }
    }

    #[test]
    fn slot_with_an_active_booking_cannot_be_reserved_again() {
        let mut slot = slot(10, SlotStatus::Open);
        let first = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        for student in [STUDENT, OTHER_STUDENT] {
            let before = slot.clone();
            assert_eq!(
                reserve(2, student, false, &mut slot, std::slice::from_ref(&first)),
                Err(SchedulingError::SlotAlreadyBooked)
            );
            assert_eq!(slot, before);
        }
        // キャンセルされた予約はスロットを確保しない
        let mut first_cancelled = first.clone();
        first_cancelled.cancel(STUDENT, &mut slot, NOW).unwrap();
        reserve(2, OTHER_STUDENT, false, &mut slot, &[first_cancelled]).unwrap();
        // 別のスロットの予約は関係ない
        let mut fresh = self::slot(12, SlotStatus::Open);
        let other = reserve(
            3,
            OTHER_STUDENT,
            false,
            &mut self::slot(11, SlotStatus::Open),
            &[],
        );
        reserve(4, STUDENT, false, &mut fresh, &[other.unwrap()]).unwrap();
    }

    #[test]
    fn takes_bookings_up_to_the_capacity_and_rejects_when_full() {
        let mut slot = slot_with_capacity(10, SlotStatus::Open, 3);
        let mut existing = Vec::new();
        for (id, student) in [(1, STUDENT), (2, OTHER_STUDENT), (3, THIRD_STUDENT)] {
            existing.push(reserve(id, student, false, &mut slot, &existing).unwrap());
        }
        assert_eq!(slot.remaining_seats(), 0);
        assert_eq!(slot.status(), SlotStatus::Open, "確定するまでは公開中");

        let before = slot.clone();
        assert_eq!(
            reserve(4, AccountId::from_u128(5), false, &mut slot, &existing),
            Err(SchedulingError::SlotAlreadyBooked)
        );
        assert_eq!(slot, before);

        // 確定した予約が定員に達したら Booked
        for booking in &mut existing {
            let student = booking.student_id();
            booking.confirm(student, &mut slot, at(1)).unwrap();
        }
        assert_eq!(slot.status(), SlotStatus::Booked);
    }

    #[test]
    fn cancelling_one_of_the_full_slot_frees_a_seat() {
        let mut slot = slot_with_capacity(10, SlotStatus::Open, 2);
        let mut existing = Vec::new();
        for (id, student) in [(1, STUDENT), (2, OTHER_STUDENT)] {
            let mut booking = reserve(id, student, false, &mut slot, &existing).unwrap();
            booking.confirm(student, &mut slot, at(1)).unwrap();
            existing.push(booking);
        }
        assert_eq!(slot.status(), SlotStatus::Booked);

        existing[0].cancel(STUDENT, &mut slot, at(2)).unwrap();
        assert_eq!(slot.status(), SlotStatus::Open);
        assert_eq!(slot.remaining_seats(), 1);
        // 空いた席は別の生徒が予約できる
        reserve(3, THIRD_STUDENT, false, &mut slot, &existing).unwrap();
        assert_eq!(slot.remaining_seats(), 0);
    }

    #[test]
    fn the_same_student_cannot_book_the_same_slot_twice() {
        let mut slot = slot_with_capacity(10, SlotStatus::Open, 3);
        let first = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        let before = slot.clone();
        assert_eq!(
            reserve(2, STUDENT, false, &mut slot, std::slice::from_ref(&first)),
            Err(SchedulingError::SlotAlreadyBookedByStudent)
        );
        assert_eq!(slot, before);
        // 別の生徒は予約できる
        reserve(
            3,
            OTHER_STUDENT,
            false,
            &mut slot,
            std::slice::from_ref(&first),
        )
        .unwrap();
        // キャンセルした後なら同じ生徒がもう一度予約できる
        let mut first_cancelled = first;
        first_cancelled.cancel(STUDENT, &mut slot, NOW).unwrap();
        reserve(4, STUDENT, false, &mut slot, &[first_cancelled]).unwrap();
    }

    #[test]
    fn trial_is_only_for_the_first_booking_of_the_student() {
        let mut first_slot = slot_with_capacity(10, SlotStatus::Open, 2);
        let mut next_slot = slot_with_capacity(11, SlotStatus::Open, 6);
        let trial = reserve(1, STUDENT, true, &mut first_slot, &[]).unwrap();
        assert_eq!(
            reserve(
                2,
                STUDENT,
                true,
                &mut next_slot,
                std::slice::from_ref(&trial)
            ),
            Err(SchedulingError::TrialAlreadyUsed)
        );
        // 通常の予約の後でも体験予約はできない
        let regular = reserve(3, OTHER_STUDENT, false, &mut first_slot, &[]).unwrap();
        assert_eq!(
            reserve(4, OTHER_STUDENT, true, &mut next_slot, &[regular]),
            Err(SchedulingError::TrialAlreadyUsed)
        );
        // 2 件目でも通常の予約はできる
        reserve(
            5,
            STUDENT,
            false,
            &mut next_slot,
            std::slice::from_ref(&trial),
        )
        .unwrap();
        // 別の生徒の予約は関係ない
        reserve(
            6,
            OTHER_STUDENT,
            true,
            &mut next_slot,
            std::slice::from_ref(&trial),
        )
        .unwrap();
        // キャンセルした予約は初回に数えない
        reserve(7, STUDENT, true, &mut next_slot, &[cancelled(trial)]).unwrap();
    }

    #[test]
    fn trial_is_not_available_after_a_completed_lesson_even_if_the_booking_is_cancelled() {
        let mut first_slot = slot_with_capacity(10, SlotStatus::Open, 2);
        let trial = reserve(1, STUDENT, true, &mut first_slot, &[]).unwrap();
        assert!(Booking::is_trial_available(STUDENT, &[], false));
        assert!(!Booking::is_trial_available(
            STUDENT,
            std::slice::from_ref(&trial),
            false
        ));
        let cancelled_trial = cancelled(trial);
        // 受けていない体験予約をキャンセルしただけなら、もう一度体験予約できる
        assert!(Booking::is_trial_available(
            STUDENT,
            std::slice::from_ref(&cancelled_trial),
            false
        ));
        // 受け終えたレッスンがあれば、予約がキャンセル済みだけでも体験は使用済み
        assert!(!Booking::is_trial_available(
            STUDENT,
            std::slice::from_ref(&cancelled_trial),
            true
        ));
        assert!(!Booking::is_trial_available(STUDENT, &[], true));
        // 別の生徒の有効な予約は関係ない
        let other = reserve(2, OTHER_STUDENT, false, &mut first_slot, &[]).unwrap();
        assert!(Booking::is_trial_available(STUDENT, &[other], false));
    }

    #[test]
    fn confirming_books_the_slot() {
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, true, &mut slot, &[]).unwrap();

        booking.confirm(STUDENT, &mut slot, at(1)).unwrap();

        assert_eq!(booking.status(), BookingStatus::Confirmed);
        assert_eq!(booking.confirmed_at(), Some(at(1)));
        assert_eq!(slot.status(), SlotStatus::Booked);
    }

    #[test]
    fn confirm_failures_change_nothing() {
        let mut open = slot(10, SlotStatus::Open);
        let pending = reserve(1, STUDENT, false, &mut open, &[]).unwrap();

        let mut confirmed = pending.clone();
        confirmed
            .confirm(STUDENT, &mut open.clone(), at(1))
            .unwrap();
        let mut closed = slot(10, SlotStatus::Closed);

        for (mut booking, student, mut slot, expected) in [
            (
                pending.clone(),
                OTHER_STUDENT,
                open.clone(),
                SchedulingError::BookingOfAnotherStudent,
            ),
            (
                pending.clone(),
                STUDENT,
                self::slot(11, SlotStatus::Open),
                SchedulingError::BookingOfAnotherSlot,
            ),
            (
                confirmed.clone(),
                STUDENT,
                open.clone(),
                SchedulingError::BookingNotPending {
                    status: BookingStatus::Confirmed,
                },
            ),
            (
                cancelled(pending.clone()),
                STUDENT,
                open.clone(),
                SchedulingError::BookingNotPending {
                    status: BookingStatus::Cancelled,
                },
            ),
            (
                pending.clone(),
                STUDENT,
                closed.clone(),
                SchedulingError::SlotNotOpen {
                    status: SlotStatus::Closed,
                },
            ),
        ] {
            let (before, slot_before) = (booking.clone(), slot.clone());
            assert_eq!(booking.confirm(student, &mut slot, at(2)), Err(expected));
            assert_eq!((booking, slot), (before, slot_before));
        }
        assert_eq!(
            closed.book(),
            Err(SchedulingError::SlotNotOpen {
                status: SlotStatus::Closed
            })
        );
    }

    #[test]
    fn status_changes_list_confirmation_and_cancellation_oldest_first() {
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        assert!(
            booking.status_changes().is_empty(),
            "作っただけでは変化はない"
        );

        booking.confirm(STUDENT, &mut slot, at(2)).unwrap();
        assert_eq!(
            booking.status_changes(),
            [BookingStatusChange::new(BookingStatus::Confirmed, at(2))]
        );

        booking.cancel(STUDENT, &mut slot, at(5)).unwrap();
        let changes = booking.status_changes();
        assert_eq!(
            changes
                .iter()
                .map(|change| (change.status(), change.changed_at()))
                .collect::<Vec<_>>(),
            [
                (BookingStatus::Confirmed, at(2)),
                (BookingStatus::Cancelled, at(5)),
            ]
        );
    }

    #[test]
    fn status_changes_of_a_booking_cancelled_before_confirmation_have_only_the_cancellation() {
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        let decision = booking.cancel(STUDENT, &mut slot, at(3)).unwrap();
        assert_eq!(
            booking.status_changes(),
            [BookingStatusChange::cancelled(
                at(3),
                Some(BookingCanceller::Student),
                None,
                Some(decision)
            )]
        );
    }

    #[test]
    fn status_changes_of_a_restored_booking_use_only_the_recorded_times() {
        let slot = slot(10, SlotStatus::Open);
        // 確定の時刻の記録がないままキャンセル済み（記録のない変化は出さない）
        let booking = Booking::restore(
            BookingId::from_u128(1),
            STUDENT,
            &slot,
            false,
            BookingStatus::Cancelled,
            at(0),
            None,
            Some(at(4)),
        );
        assert_eq!(
            booking.status_changes(),
            [BookingStatusChange::cancelled(
                at(4),
                Some(BookingCanceller::Student),
                None,
                None
            )]
        );
        // 記録の時刻が前後していても古い順に並べる
        let booking = Booking::restore(
            BookingId::from_u128(2),
            STUDENT,
            &slot,
            false,
            BookingStatus::Cancelled,
            at(0),
            Some(at(6)),
            Some(at(4)),
        );
        assert_eq!(
            booking.status_changes(),
            [
                BookingStatusChange::cancelled(at(4), Some(BookingCanceller::Student), None, None),
                BookingStatusChange::new(BookingStatus::Confirmed, at(6)),
            ]
        );
    }

    #[test]
    fn restore_round_trips_a_confirmed_booking() {
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, true, &mut slot, &[]).unwrap();
        booking.confirm(STUDENT, &mut slot, at(1)).unwrap();

        let restored = Booking::restore(
            booking.id(),
            STUDENT,
            &slot,
            true,
            BookingStatus::Confirmed,
            NOW,
            Some(at(1)),
            None,
        );

        assert_eq!(restored, booking);
    }

    fn lesson_menu(id: u128, coach: AccountId, is_published: bool) -> LessonMenu {
        use crate::catalog::{
            LessonDuration, LessonFormat, LessonMenuContent, LessonMenuDescription, LessonMenuName,
        };
        LessonMenu::restore(
            crate::catalog::LessonMenuId::from_u128(id),
            coach,
            LessonMenuContent {
                sport: Sport::Tennis,
                format: LessonFormat::Live(LessonDuration::from_minutes(60).unwrap()),
                name: LessonMenuName::new("フォア改善").unwrap(),
                description: LessonMenuDescription::new("").unwrap(),
            },
            is_published,
            NOW,
        )
    }

    /// 公開中のメニュー 50 に結びついた、開始 10 時・1 時間の公開中の枠。
    fn slot_with_menu() -> Slot {
        let open = slot(10, SlotStatus::Open);
        Slot::publish(
            open.id(),
            COACH,
            Sport::Tennis,
            open.starts_at(),
            open.ends_at(),
            SlotCapacity::ONE,
        )
        .unwrap()
        .with_lesson_menu(&lesson_menu(50, COACH, true))
        .unwrap()
    }

    #[test]
    fn reserved_booking_has_no_lesson_menu() {
        let mut slot = slot_with_menu();
        let booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        assert_eq!(booking.lesson_menu(), None);
    }

    #[test]
    fn booking_copies_the_published_lesson_menu_of_its_slot() {
        let mut slot = slot_with_menu();
        let menu = lesson_menu(50, COACH, true);
        let booking = reserve(1, STUDENT, false, &mut slot, &[])
            .unwrap()
            .with_lesson_menu(&slot, Some(&menu));
        assert_eq!(
            booking.lesson_menu(),
            Some(&BookingLessonMenu::copy_of(&menu))
        );
        assert_eq!(booking.status(), BookingStatus::Pending);

        // 予約した後にメニューが直されても、写しは変わらない（確定・キャンセルでも残る）
        let mut confirmed = booking.clone();
        confirmed.confirm(STUDENT, &mut slot, at(1)).unwrap();
        assert_eq!(confirmed.lesson_menu(), booking.lesson_menu());
        confirmed.cancel(STUDENT, &mut slot, at(2)).unwrap();
        assert_eq!(confirmed.lesson_menu(), booking.lesson_menu());
    }

    #[test]
    fn booking_without_a_copyable_menu_has_no_lesson_menu() {
        let with_menu = slot_with_menu();
        let without_menu = slot(10, SlotStatus::Open);
        for (slot, menu) in [
            // 公開中でない（予約は断らず、メニューなしの予約）
            (with_menu.clone(), Some(lesson_menu(50, COACH, false))),
            // 枠に結びついたメニューでない
            (with_menu.clone(), Some(lesson_menu(51, COACH, true))),
            // catalog から引けない
            (with_menu.clone(), None),
            // 枠にメニューがない
            (without_menu, Some(lesson_menu(50, COACH, true))),
        ] {
            let mut reserved_slot = slot.clone();
            let booking = reserve(1, STUDENT, false, &mut reserved_slot, &[])
                .unwrap()
                .with_lesson_menu(&slot, menu.as_ref());
            assert_eq!(booking.lesson_menu(), None, "{menu:?}");
        }

        // 別のスロット
        let mut slot = with_menu;
        let other = Slot::publish(
            SlotId::from_u128(11),
            COACH,
            Sport::Tennis,
            at(11),
            at(12),
            SlotCapacity::ONE,
        )
        .unwrap()
        .with_lesson_menu(&lesson_menu(50, COACH, true))
        .unwrap();
        let booking = reserve(1, STUDENT, false, &mut slot, &[])
            .unwrap()
            .with_lesson_menu(&other, Some(&lesson_menu(50, COACH, true)));
        assert_eq!(booking.lesson_menu(), None);
    }

    #[test]
    fn matches_coach_only_the_coach_of_the_slot_or_any_without_a_coach() {
        let mut slot = slot(10, SlotStatus::Open);
        let booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        assert!(
            booking.matches_coach(None),
            "指定がなければどのコーチの予約も"
        );
        assert!(booking.matches_coach(Some(COACH)));
        // 別のコーチ・予約した生徒（コーチでない ID）には当てはまらない
        assert!(!booking.matches_coach(Some(AccountId::from_u128(99))));
        assert!(!booking.matches_coach(Some(STUDENT)));
        // キャンセルしても枠のコーチは変わらない
        assert!(cancelled(booking).matches_coach(Some(COACH)));
    }

    #[test]
    fn matches_student_only_the_student_who_booked_or_any_without_a_student() {
        let mut slot = slot(10, SlotStatus::Open);
        let booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        assert!(
            booking.matches_student(None),
            "指定がなければどの生徒の予約も"
        );
        assert!(booking.matches_student(Some(STUDENT)));
        // 別の生徒・枠のコーチ（生徒でない ID）には当てはまらない
        assert!(!booking.matches_student(Some(AccountId::from_u128(99))));
        assert!(!booking.matches_student(Some(COACH)));
        // キャンセルしても予約した生徒は変わらない
        assert!(cancelled(booking).matches_student(Some(STUDENT)));
    }

    fn account(id: AccountId, role: crate::identity::Role, sports: &[Sport]) -> Account {
        Account::new(
            id,
            crate::identity::Email::new(format!("user{}@example.com", id.as_u128())).unwrap(),
            crate::profile::DisplayName::new("たろう").unwrap(),
            role,
            sports.iter().copied(),
            NOW,
        )
        .unwrap()
    }

    #[test]
    fn matches_coach_sport_by_the_sports_the_coach_of_the_slot_offers() {
        use crate::identity::Role;
        let mut slot = slot(10, SlotStatus::Open);
        let booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        // 枠はテニスだが、コーチはテニスとゴルフを提供する
        let coach = account(COACH, Role::Coach, &[Sport::Tennis, Sport::Golf]);
        let filter = |sport| CoachSportFilter::new(Some(sport));
        assert!(booking.matches_coach_sport(filter(Sport::Tennis), Some(&coach)));
        assert!(
            booking.matches_coach_sport(filter(Sport::Golf), Some(&coach)),
            "枠のスポーツでなくコーチの提供スポーツで決める"
        );
        assert!(!booking.matches_coach_sport(filter(Sport::Soccer), Some(&coach)));
        // アカウントが引けない予約・枠のコーチでないアカウント・コーチでないアカウントは当てはまらない
        assert!(!booking.matches_coach_sport(filter(Sport::Tennis), None));
        let other = account(AccountId::from_u128(99), Role::Coach, &[Sport::Tennis]);
        assert!(!booking.matches_coach_sport(filter(Sport::Tennis), Some(&other)));
        let student = account(COACH, Role::Student, &[Sport::Tennis]);
        assert!(!booking.matches_coach_sport(filter(Sport::Tennis), Some(&student)));
        // キャンセルしても枠のコーチは変わらない
        assert!(cancelled(booking).matches_coach_sport(filter(Sport::Golf), Some(&coach)));
    }

    #[test]
    fn matches_coach_sport_of_a_cancelled_booking_is_the_same_as_before_the_cancellation() {
        use crate::identity::Role;
        let mut slot = slot(10, SlotStatus::Open);
        let mut booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        let before = booking.clone();
        booking.cancel(STUDENT, &mut slot, at(8)).unwrap();
        assert_eq!(booking.status(), BookingStatus::Cancelled);
        let coach = account(COACH, Role::Coach, &[Sport::Golf]);
        // キャンセルされた予約の一覧が使う。枠のスポーツ（テニス）でなく、キャンセルの前と同じくコーチの提供スポーツで決める
        for sport in [Sport::Tennis, Sport::Golf, Sport::Soccer, Sport::Swimming] {
            let filter = CoachSportFilter::new(Some(sport));
            assert_eq!(
                booking.matches_coach_sport(filter, Some(&coach)),
                before.matches_coach_sport(filter, Some(&coach)),
                "{sport:?}"
            );
            assert!(!booking.matches_coach_sport(filter, None));
        }
        assert!(
            booking.matches_coach_sport(CoachSportFilter::new(Some(Sport::Golf)), Some(&coach))
        );
    }

    #[test]
    fn matches_coach_sport_without_a_sport_is_every_booking() {
        let mut slot = slot(10, SlotStatus::Open);
        let booking = reserve(1, STUDENT, false, &mut slot, &[]).unwrap();
        assert!(booking.matches_coach_sport(CoachSportFilter::default(), None));
        let coach = account(COACH, crate::identity::Role::Coach, &[Sport::Soccer]);
        assert!(booking.matches_coach_sport(CoachSportFilter::default(), Some(&coach)));
    }

    #[test]
    fn matches_lesson_menu_only_the_copied_menu_or_any_without_a_menu() {
        let mut with_menu = slot_with_menu();
        let menu = LessonMenuId::from_u128(50);
        let booking = reserve(1, STUDENT, false, &mut with_menu, &[])
            .unwrap()
            .with_lesson_menu(&with_menu, Some(&lesson_menu(50, COACH, true)));
        assert!(
            booking.matches_lesson_menu(None),
            "指定がなければどのメニューの予約も"
        );
        assert!(booking.matches_lesson_menu(Some(menu)));
        // 別のメニュー・ない ID には当てはまらない
        assert!(!booking.matches_lesson_menu(Some(LessonMenuId::from_u128(99))));
        // キャンセルしてもメニューの写しは変わらない
        assert!(cancelled(booking).matches_lesson_menu(Some(menu)));

        // メニューの写しのない予約は、指定があればどのメニューにも当てはまらない
        let mut without = slot(11, SlotStatus::Open);
        let booking = reserve(2, STUDENT, false, &mut without, &[]).unwrap();
        assert!(booking.matches_lesson_menu(None));
        assert!(!booking.matches_lesson_menu(Some(menu)));
    }

    #[test]
    fn matches_canceller_only_the_canceller_or_any_without_one() {
        let mut first = slot(10, SlotStatus::Open);
        let pending = reserve(1, STUDENT, false, &mut first, &[]).unwrap();
        assert!(pending.matches_canceller(None), "指定がなければどの予約も");
        // キャンセルしていない予約は、指定があればどの指定にも当てはまらない
        assert!(!pending.matches_canceller(Some(BookingCanceller::Student)));
        assert!(!pending.matches_canceller(Some(BookingCanceller::Headquarters)));

        let by_student = cancelled(pending);
        assert!(by_student.matches_canceller(None));
        assert!(by_student.matches_canceller(Some(BookingCanceller::Student)));
        assert!(!by_student.matches_canceller(Some(BookingCanceller::Coach)));
        assert!(!by_student.matches_canceller(Some(BookingCanceller::Headquarters)));

        let mut other = slot(11, SlotStatus::Open);
        let mut by_headquarters = reserve(2, STUDENT, false, &mut other, &[]).unwrap();
        by_headquarters
            .cancel_by_headquarters(&mut other, NOW)
            .unwrap();
        assert!(by_headquarters.matches_canceller(Some(BookingCanceller::Headquarters)));
        assert!(!by_headquarters.matches_canceller(Some(BookingCanceller::Student)));
    }

    #[test]
    fn restored_lesson_menu_is_kept() {
        let mut slot = slot_with_menu();
        let copy = BookingLessonMenu::copy_of(&lesson_menu(50, COACH, true));
        let booking = reserve(1, STUDENT, false, &mut slot, &[])
            .unwrap()
            .with_restored_lesson_menu(Some(copy.clone()));
        assert_eq!(booking.lesson_menu(), Some(&copy));
        assert_eq!(booking.with_restored_lesson_menu(None).lesson_menu(), None);
    }
}
