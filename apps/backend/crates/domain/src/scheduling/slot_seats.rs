use super::SlotCapacity;

/// スロットの席（定員と、有効な予約の数）。有効な予約は未確定（`Pending`）か確定（`Confirmed`）の予約で、
/// キャンセルした予約は数えない。
///
/// 数を動かすのはスロット（[`super::Slot`]）の予約・確定・キャンセルだけ。保存先は数を持たず、予約の行から
/// 数えて [`SlotSeats::restore`] に渡す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SlotSeats {
    capacity: SlotCapacity,
    pending: u32,
    confirmed: u32,
}

impl SlotSeats {
    /// 予約のない席。
    pub const fn vacant(capacity: SlotCapacity) -> Self {
        Self {
            capacity,
            pending: 0,
            confirmed: 0,
        }
    }

    /// 保存先から数えた席を組み立てる（保存先の実装用）。数が定員を超えていても確かめない
    /// （予約の時点で確かめた。残りの席は 0 として読む）。
    pub const fn restore(capacity: SlotCapacity, pending: u32, confirmed: u32) -> Self {
        Self {
            capacity,
            pending,
            confirmed,
        }
    }

    pub const fn capacity(self) -> SlotCapacity {
        self.capacity
    }

    /// 未確定の予約の数。
    pub const fn pending(self) -> u32 {
        self.pending
    }

    /// 確定した予約の数。
    pub const fn confirmed(self) -> u32 {
        self.confirmed
    }

    /// 有効な予約（未確定か確定）の数。
    pub const fn active(self) -> u32 {
        self.pending + self.confirmed
    }

    /// 残りの席（定員 − 有効な予約の数。0 未満にはしない）。
    pub const fn remaining(self) -> u32 {
        (self.capacity.value() as u32).saturating_sub(self.active())
    }

    /// 確定した予約が定員に達しているか。
    pub const fn is_fully_confirmed(self) -> bool {
        self.confirmed >= self.capacity.value() as u32
    }

    /// 未確定の予約を 1 つ足す。
    pub(super) const fn hold(&mut self) {
        self.pending += 1;
    }

    /// 未確定の予約を 1 つ確定に移す。
    pub(super) const fn confirm(&mut self) {
        self.pending = self.pending.saturating_sub(1);
        self.confirmed += 1;
    }

    /// 未確定の予約を 1 つ減らす（キャンセル）。
    pub(super) const fn release_pending(&mut self) {
        self.pending = self.pending.saturating_sub(1);
    }

    /// 確定した予約を 1 つ減らす（キャンセル）。
    pub(super) const fn release_confirmed(&mut self) {
        self.confirmed = self.confirmed.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capacity(value: i64) -> SlotCapacity {
        SlotCapacity::new(value).unwrap()
    }

    #[test]
    fn vacant_seats_have_all_remaining() {
        let seats = SlotSeats::vacant(capacity(3));
        assert_eq!((seats.active(), seats.remaining()), (0, 3));
        assert!(!seats.is_fully_confirmed());
        assert_eq!(SlotSeats::default(), SlotSeats::vacant(SlotCapacity::ONE));
    }

    #[test]
    fn remaining_counts_pending_and_confirmed() {
        let seats = SlotSeats::restore(capacity(3), 1, 1);
        assert_eq!((seats.active(), seats.remaining()), (2, 1));
        assert!(!seats.is_fully_confirmed());
        assert!(SlotSeats::restore(capacity(3), 0, 3).is_fully_confirmed());
        // 数が定員を超えていても残りは 0
        assert_eq!(SlotSeats::restore(capacity(1), 2, 0).remaining(), 0);
    }

    #[test]
    fn hold_confirm_and_release_move_the_counts() {
        let mut seats = SlotSeats::vacant(capacity(2));
        seats.hold();
        seats.hold();
        assert_eq!((seats.pending(), seats.confirmed()), (2, 0));
        seats.confirm();
        assert_eq!((seats.pending(), seats.confirmed()), (1, 1));
        seats.release_pending();
        seats.release_confirmed();
        assert_eq!(seats, SlotSeats::vacant(capacity(2)));
    }
}
