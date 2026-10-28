use domain::scheduling::Slot;

/// コーチの自分の枠の一覧（[`super::ListCoachSlots`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachSlotList {
    /// コーチの枠（状態を問わない。終わった枠も含む）と、その枠に有効な予約（未確定か確定。
    /// [`domain::scheduling::BookingStatus::is_active`]）が入っているか。開始時刻の古い順で、同じ開始時刻は枠の ID の順
    pub items: Vec<(Slot, bool)>,
}
