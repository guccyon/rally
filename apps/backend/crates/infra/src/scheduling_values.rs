//! scheduling の値と、DB に保存する文字列の対応表。
//!
//! `scheduling.slots.status`（CHECK 制約の値）はこの表だけを通す。`sport` は
//! [`crate::identity_values::sport_value`] と同じ値。CHECK との一致は `tests/schema.rs` が確かめる。

use domain::scheduling::SlotStatus;

/// `scheduling.slots.status` の値。
pub const fn slot_status_value(status: SlotStatus) -> &'static str {
    match status {
        SlotStatus::Open => "open",
        SlotStatus::Booked => "booked",
        SlotStatus::Closed => "closed",
    }
}

pub fn slot_status_from_value(value: &str) -> Option<SlotStatus> {
    SlotStatus::ALL
        .into_iter()
        .find(|status| slot_status_value(*status) == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        for status in SlotStatus::ALL {
            assert_eq!(
                slot_status_from_value(slot_status_value(status)),
                Some(status)
            );
        }
    }

    #[test]
    fn unknown_values_are_none() {
        for value in ["", "Open", "cancelled", " open"] {
            assert_eq!(slot_status_from_value(value), None, "{value:?}");
        }
    }
}
