use std::time::{Duration, SystemTime};

use super::{SlotCapacity, SlotId, SlotSeats, SlotStatus};
use crate::{
    DomainError,
    catalog::{LessonFormat, LessonMenu, LessonMenuId},
    identity::{AccountId, Sport},
};

/// スロット（glossary の slot）。コーチが公開する予約枠で、開始は終了より前。
///
/// 期間は開始を含み終了を含まない（`[starts_at, ends_at)`）。前の枠の終了と次の枠の開始が
/// 同じ時刻なら重ならない。
///
/// 定員（[`SlotCapacity`]。1〜6）までの有効な予約（未確定か確定）を受ける。確定した予約が定員に達すると
/// [`SlotStatus::Booked`] になり、キャンセルで空きができると [`SlotStatus::Open`] に戻る。定員 1 の枠は
/// マンツーマン（予約の確定で埋まり、確定した予約のキャンセルで公開中に戻る）。
///
/// 任意でコーチのレッスンメニュー（catalog の [`LessonMenu`]）に結びつく（[`Slot::with_lesson_menu`]）。持つのは
/// メニューの ID だけで、メニューの中身（名前・時間）の持ち主は catalog。結びつけるのは公開のときだけ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    id: SlotId,
    coach_id: AccountId,
    sport: Sport,
    starts_at: SystemTime,
    ends_at: SystemTime,
    status: SlotStatus,
    seats: SlotSeats,
    lesson_menu_id: Option<LessonMenuId>,
}

impl Slot {
    /// 公開する枠を作る（状態は [`SlotStatus::Open`]、予約はない）。開始が終了より前でなければ
    /// [`DomainError::SlotEndsBeforeItStarts`]。同じコーチの枠との重なりは [`super::CoachCalendar`] が判定する。
    pub fn publish(
        id: SlotId,
        coach_id: AccountId,
        sport: Sport,
        starts_at: SystemTime,
        ends_at: SystemTime,
        capacity: SlotCapacity,
    ) -> Result<Self, DomainError> {
        Self::restore(
            id,
            coach_id,
            sport,
            starts_at,
            ends_at,
            SlotStatus::Open,
            SlotSeats::vacant(capacity),
        )
    }

    /// 保存した枠を読み戻す（状態を問わない）。開始と終了の検証は [`Slot::publish`] と同じ。
    /// `seats` は保存先が予約の行から数えた席（[`SlotSeats::restore`]）。状態と席が合わなければ
    /// [`DomainError::SlotStatusMismatchesSeats`]（予約済みなら確定した予約が定員ちょうど、公開中なら定員未満。
    /// 閉じた枠は問わない）。
    pub fn restore(
        id: SlotId,
        coach_id: AccountId,
        sport: Sport,
        starts_at: SystemTime,
        ends_at: SystemTime,
        status: SlotStatus,
        seats: SlotSeats,
    ) -> Result<Self, DomainError> {
        if starts_at >= ends_at {
            return Err(DomainError::SlotEndsBeforeItStarts);
        }
        let consistent = match status {
            SlotStatus::Open => !seats.is_fully_confirmed(),
            SlotStatus::Booked => seats.is_fully_confirmed(),
            SlotStatus::Closed => true,
        };
        if !consistent {
            return Err(DomainError::SlotStatusMismatchesSeats { status });
        }
        Ok(Self {
            id,
            coach_id,
            sport,
            starts_at,
            ends_at,
            status,
            seats,
            lesson_menu_id: None,
        })
    }

    /// 公開する枠（[`Slot::publish`] で作った枠）をレッスンメニュー `menu` に結びつける。
    ///
    /// 結びつけられるのは、枠のコーチ自身の・公開中の・同期のメニューで、スポーツが枠と同じで、1 回の時間が枠の長さと
    /// 同じもの。違えば順に [`DomainError::LessonMenuOfAnotherCoach`]・[`DomainError::LessonMenuNotPublished`]・
    /// [`DomainError::LessonMenuNotLive`]・[`DomainError::LessonMenuSportMismatch`]・
    /// [`DomainError::SlotLengthMismatchesLessonMenu`]。
    pub fn with_lesson_menu(mut self, menu: &LessonMenu) -> Result<Self, DomainError> {
        if menu.coach_id() != self.coach_id {
            return Err(DomainError::LessonMenuOfAnotherCoach);
        }
        if !menu.is_published() {
            return Err(DomainError::LessonMenuNotPublished);
        }
        let LessonFormat::Live(duration) = menu.format() else {
            return Err(DomainError::LessonMenuNotLive);
        };
        if menu.sport() != self.sport {
            return Err(DomainError::LessonMenuSportMismatch {
                menu: menu.sport(),
                slot: self.sport,
            });
        }
        let minutes = Duration::from_secs(u64::from(duration.minutes()) * 60);
        if self.ends_at.duration_since(self.starts_at).ok() != Some(minutes) {
            return Err(DomainError::SlotLengthMismatchesLessonMenu {
                minutes: duration.minutes(),
            });
        }
        self.lesson_menu_id = Some(menu.id());
        Ok(self)
    }

    /// 保存先から読んだ枠に、結びついていたレッスンメニューの ID を戻す（保存先の実装用。結びつけたときに
    /// [`Slot::with_lesson_menu`] で確かめているので、ここでは確かめない）。
    pub const fn with_restored_lesson_menu(mut self, lesson_menu_id: Option<LessonMenuId>) -> Self {
        self.lesson_menu_id = lesson_menu_id;
        self
    }

    pub const fn id(&self) -> SlotId {
        self.id
    }

    pub const fn coach_id(&self) -> AccountId {
        self.coach_id
    }

    pub const fn sport(&self) -> Sport {
        self.sport
    }

    pub const fn starts_at(&self) -> SystemTime {
        self.starts_at
    }

    pub const fn ends_at(&self) -> SystemTime {
        self.ends_at
    }

    pub const fn status(&self) -> SlotStatus {
        self.status
    }

    pub const fn capacity(&self) -> SlotCapacity {
        self.seats.capacity()
    }

    /// 結びついたレッスンメニュー。なければ `None`。
    pub const fn lesson_menu_id(&self) -> Option<LessonMenuId> {
        self.lesson_menu_id
    }

    /// 定員と有効な予約の数。
    pub const fn seats(&self) -> SlotSeats {
        self.seats
    }

    /// 残りの席（定員 − 有効な予約の数）。
    pub const fn remaining_seats(&self) -> u32 {
        self.seats.remaining()
    }

    /// 予約を受けられるか。公開中でなければ [`DomainError::SlotNotOpen`]、残りの席がなければ
    /// [`DomainError::SlotAlreadyBooked`]（満席）。何も変えない。
    pub fn ensure_seat_available(&self) -> Result<(), DomainError> {
        if self.status != SlotStatus::Open {
            return Err(DomainError::SlotNotOpen {
                status: self.status,
            });
        }
        if self.remaining_seats() == 0 {
            return Err(DomainError::SlotAlreadyBooked);
        }
        Ok(())
    }

    /// 未確定の予約で席を 1 つ押さえる（状態は変えない）。受けられなければ [`Slot::ensure_seat_available`] の
    /// エラーで、何も変えない。予約の作成（[`super::Booking::reserve`]）から呼ぶ。
    pub(super) fn hold(&mut self) -> Result<(), DomainError> {
        self.ensure_seat_available()?;
        self.seats.hold();
        Ok(())
    }

    /// 予約の確定で席を確定に移す。確定した予約が定員に達したら [`SlotStatus::Open`] → [`SlotStatus::Booked`]。
    /// 公開中でなければ [`DomainError::SlotNotOpen`] で、何も変えない。予約の確定（[`super::Booking::confirm`]）から呼ぶ。
    pub(super) fn book(&mut self) -> Result<(), DomainError> {
        if self.status != SlotStatus::Open {
            return Err(DomainError::SlotNotOpen {
                status: self.status,
            });
        }
        self.seats.confirm();
        if self.seats.is_fully_confirmed() {
            self.status = SlotStatus::Booked;
        }
        Ok(())
    }

    /// 未確定の予約のキャンセルで押さえた席を空ける（状態は変えない）。予約のキャンセル
    /// （[`super::Booking::cancel`]）から呼ぶ。
    pub(super) fn release_hold(&mut self) {
        self.seats.release_pending();
    }

    /// 確定した予約のキャンセルで席を空ける。予約済み（[`SlotStatus::Booked`]）なら公開中に戻す。
    /// 確定した予約が入っていない（または閉じた枠）なら [`DomainError::SlotNotBooked`] で、何も変えない。
    /// 予約のキャンセル（[`super::Booking::cancel`]）から呼ぶ。
    pub(super) fn release(&mut self) -> Result<(), DomainError> {
        if self.seats.confirmed() == 0 || self.status == SlotStatus::Closed {
            return Err(DomainError::SlotNotBooked {
                status: self.status,
            });
        }
        self.seats.release_confirmed();
        self.status = SlotStatus::Open;
        Ok(())
    }

    /// 期間が重なるか（コーチ・状態は見ない）。
    pub fn overlaps(&self, other: &Slot) -> bool {
        self.starts_at < other.ends_at && other.starts_at < self.ends_at
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn slot_with_capacity(id: u128, from: u64, to: u64, capacity: i64) -> Slot {
        Slot::publish(
            SlotId::from_u128(id),
            AccountId::from_u128(1),
            Sport::Tennis,
            at(from),
            at(to),
            SlotCapacity::new(capacity).unwrap(),
        )
        .unwrap()
    }

    fn slot(id: u128, from: u64, to: u64) -> Slot {
        slot_with_capacity(id, from, to, 1)
    }

    #[test]
    fn published_slot_is_open_and_vacant() {
        let slot = slot(1, 10, 11);
        assert_eq!(slot.status(), SlotStatus::Open);
        assert_eq!(slot.capacity(), SlotCapacity::ONE);
        assert_eq!(slot.remaining_seats(), 1);
        assert_eq!((slot.starts_at(), slot.ends_at()), (at(10), at(11)));

        let group = slot_with_capacity(2, 10, 11, 3);
        assert_eq!(group.capacity().value(), 3);
        assert_eq!(group.remaining_seats(), 3);
    }

    #[test]
    fn slot_must_start_before_it_ends() {
        for (from, to) in [(10, 10), (11, 10)] {
            assert_eq!(
                Slot::publish(
                    SlotId::from_u128(1),
                    AccountId::from_u128(1),
                    Sport::Tennis,
                    at(from),
                    at(to),
                    SlotCapacity::ONE,
                ),
                Err(DomainError::SlotEndsBeforeItStarts)
            );
        }
    }

    #[test]
    fn booking_fills_only_an_open_slot() {
        let mut slot = slot(1, 10, 11);
        slot.hold().unwrap();
        slot.book().unwrap();
        assert_eq!(slot.status(), SlotStatus::Booked);
        assert_eq!(
            slot.book(),
            Err(DomainError::SlotNotOpen {
                status: SlotStatus::Booked
            })
        );
        assert_eq!(slot.status(), SlotStatus::Booked);
        assert_eq!(slot.seats().confirmed(), 1);
    }

    #[test]
    fn releasing_reopens_only_a_slot_with_a_confirmed_booking() {
        let mut slot = slot(1, 10, 11);
        assert_eq!(
            slot.release(),
            Err(DomainError::SlotNotBooked {
                status: SlotStatus::Open
            })
        );
        slot.hold().unwrap();
        slot.book().unwrap();
        slot.release().unwrap();
        assert_eq!(slot.status(), SlotStatus::Open);
        assert_eq!(slot.remaining_seats(), 1);
    }

    #[test]
    fn holds_seats_up_to_the_capacity_and_rejects_when_full() {
        let mut slot = slot_with_capacity(1, 10, 11, 3);
        for remaining in [2, 1, 0] {
            slot.hold().unwrap();
            assert_eq!(slot.remaining_seats(), remaining);
        }
        // 未確定の予約で席が埋まっても、確定するまでは公開中
        assert_eq!(slot.status(), SlotStatus::Open);
        let before = slot.clone();
        assert_eq!(slot.hold(), Err(DomainError::SlotAlreadyBooked));
        assert_eq!(slot, before);
    }

    #[test]
    fn slot_is_booked_when_confirmed_bookings_reach_the_capacity() {
        let mut slot = slot_with_capacity(1, 10, 11, 3);
        for _ in 0..3 {
            slot.hold().unwrap();
        }
        slot.book().unwrap();
        slot.book().unwrap();
        assert_eq!(slot.status(), SlotStatus::Open, "確定 2 / 定員 3");
        slot.book().unwrap();
        assert_eq!(slot.status(), SlotStatus::Booked);
        assert_eq!(slot.seats().confirmed(), 3);
    }

    #[test]
    fn cancelling_frees_a_seat_and_reopens_the_slot() {
        let mut slot = slot_with_capacity(1, 10, 11, 2);
        slot.hold().unwrap();
        slot.hold().unwrap();
        slot.book().unwrap();
        slot.book().unwrap();
        assert_eq!(slot.status(), SlotStatus::Booked);

        slot.release().unwrap();
        assert_eq!(slot.status(), SlotStatus::Open);
        assert_eq!(slot.remaining_seats(), 1);
        // 空いた席はまた予約できる
        slot.hold().unwrap();
        assert_eq!(slot.remaining_seats(), 0);
        // 未確定の予約のキャンセルでも席が空く（状態は変えない）
        slot.release_hold();
        assert_eq!(slot.remaining_seats(), 1);
        assert_eq!(slot.status(), SlotStatus::Open);
    }

    #[test]
    fn closed_slot_takes_no_bookings_and_cannot_be_released() {
        let mut closed = Slot::restore(
            SlotId::from_u128(1),
            AccountId::from_u128(1),
            Sport::Tennis,
            at(10),
            at(11),
            SlotStatus::Closed,
            SlotSeats::restore(SlotCapacity::ONE, 0, 1).unwrap(),
        )
        .unwrap();
        let before = closed.clone();
        assert_eq!(
            closed.hold(),
            Err(DomainError::SlotNotOpen {
                status: SlotStatus::Closed
            })
        );
        assert_eq!(
            closed.release(),
            Err(DomainError::SlotNotBooked {
                status: SlotStatus::Closed
            })
        );
        assert_eq!(closed, before);
    }

    #[test]
    fn restored_status_must_match_the_seats() {
        let restore = |status, pending, confirmed| {
            Slot::restore(
                SlotId::from_u128(1),
                AccountId::from_u128(1),
                Sport::Tennis,
                at(10),
                at(11),
                status,
                SlotSeats::restore(SlotCapacity::new(2).unwrap(), pending, confirmed).unwrap(),
            )
        };
        for (status, pending, confirmed) in [
            (SlotStatus::Open, 0, 0),
            (SlotStatus::Open, 2, 0),
            (SlotStatus::Open, 1, 1),
            (SlotStatus::Booked, 0, 2),
            (SlotStatus::Closed, 0, 0),
            (SlotStatus::Closed, 0, 2),
        ] {
            let slot = restore(status, pending, confirmed).unwrap();
            assert_eq!(slot.status(), status);
        }
        for (status, pending, confirmed) in [
            (SlotStatus::Open, 0, 2),
            (SlotStatus::Booked, 0, 0),
            (SlotStatus::Booked, 1, 1),
        ] {
            assert_eq!(
                restore(status, pending, confirmed),
                Err(DomainError::SlotStatusMismatchesSeats { status }),
                "{status:?} {pending}/{confirmed}"
            );
        }
    }

    fn menu(coach: u128, sport: Sport, format: LessonFormat, is_published: bool) -> LessonMenu {
        LessonMenu::restore(
            LessonMenuId::from_u128(50),
            AccountId::from_u128(coach),
            crate::catalog::LessonMenuContent {
                sport,
                format,
                name: crate::catalog::LessonMenuName::new("フォア改善").unwrap(),
                description: crate::catalog::LessonMenuDescription::default(),
            },
            is_published,
            SystemTime::UNIX_EPOCH,
        )
    }

    fn live(minutes: i64) -> LessonFormat {
        LessonFormat::Live(crate::catalog::LessonDuration::from_minutes(minutes).unwrap())
    }

    #[test]
    fn published_slot_has_no_lesson_menu() {
        assert_eq!(slot(1, 10, 11).lesson_menu_id(), None);
    }

    #[test]
    fn slot_is_bound_to_a_published_live_menu_of_its_coach_sport_and_length() {
        let bound = slot(1, 10, 11)
            .with_lesson_menu(&menu(1, Sport::Tennis, live(60), true))
            .unwrap();
        assert_eq!(bound.lesson_menu_id(), Some(LessonMenuId::from_u128(50)));
        assert_eq!(bound.status(), SlotStatus::Open);
        assert_eq!(bound.remaining_seats(), 1);

        // 90 分の枠と 90 分のメニュー
        let ninety = Slot::publish(
            SlotId::from_u128(2),
            AccountId::from_u128(1),
            Sport::Tennis,
            at(10),
            at(10) + Duration::from_secs(90 * 60),
            SlotCapacity::ONE,
        )
        .unwrap()
        .with_lesson_menu(&menu(1, Sport::Tennis, live(90), true))
        .unwrap();
        assert_eq!(ninety.lesson_menu_id(), Some(LessonMenuId::from_u128(50)));
    }

    #[test]
    fn menu_that_does_not_fit_the_slot_is_rejected() {
        for (menu, expected) in [
            (
                menu(2, Sport::Tennis, live(60), true),
                DomainError::LessonMenuOfAnotherCoach,
            ),
            (
                menu(1, Sport::Tennis, live(60), false),
                DomainError::LessonMenuNotPublished,
            ),
            (
                menu(1, Sport::Tennis, LessonFormat::Async, true),
                DomainError::LessonMenuNotLive,
            ),
            (
                menu(1, Sport::Swimming, live(60), true),
                DomainError::LessonMenuSportMismatch {
                    menu: Sport::Swimming,
                    slot: Sport::Tennis,
                },
            ),
            (
                menu(1, Sport::Tennis, live(45), true),
                DomainError::SlotLengthMismatchesLessonMenu { minutes: 45 },
            ),
            (
                menu(1, Sport::Tennis, live(120), true),
                DomainError::SlotLengthMismatchesLessonMenu { minutes: 120 },
            ),
        ] {
            assert_eq!(slot(1, 10, 11).with_lesson_menu(&menu), Err(expected));
        }
    }

    #[test]
    fn restored_lesson_menu_is_kept() {
        let restored = slot(1, 10, 11).with_restored_lesson_menu(Some(LessonMenuId::from_u128(9)));
        assert_eq!(restored.lesson_menu_id(), Some(LessonMenuId::from_u128(9)));
        assert_eq!(
            restored.with_restored_lesson_menu(None).lesson_menu_id(),
            None
        );
    }

    #[test]
    fn overlap_is_half_open() {
        let base = slot(1, 10, 12);
        assert!(base.overlaps(&slot(2, 11, 13)));
        assert!(base.overlaps(&slot(2, 9, 11)));
        assert!(base.overlaps(&slot(2, 10, 12)));
        assert!(base.overlaps(&slot(2, 10, 11)));
        assert!(base.overlaps(&slot(2, 9, 13)));
        // 終了と開始が同じ時刻なら重ならない
        assert!(!base.overlaps(&slot(2, 12, 13)));
        assert!(!base.overlaps(&slot(2, 8, 10)));
    }
}
