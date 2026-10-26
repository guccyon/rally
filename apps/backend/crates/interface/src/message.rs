//! 利用者に見せる文言（API の `message`）。domain の値やエラーの変種から組み立てる。

use application::scheduling::slot_id_uuid;
use domain::{
    DomainError,
    identity::Role,
    scheduling::{BookingStatus, SlotStatus},
};

/// 役割の呼び名。
pub(crate) const fn role_label(role: Role) -> &'static str {
    match role {
        Role::Student => "生徒",
        Role::Coach => "コーチ",
        Role::Headquarters => "本部",
    }
}

/// 不変条件の違反の説明。
pub(crate) fn domain_error(error: &DomainError) -> String {
    match error {
        DomainError::EmptyDisplayName => "表示名が空".to_string(),
        DomainError::DisplayNameTooLong { max } => format!("表示名は {max} 文字まで"),
        DomainError::InvalidEmail => "メールアドレスの形が不正".to_string(),
        DomainError::RoleAttributeMismatch { role, sports } => {
            format!("{}にスポーツ {sports} 個は持てない", role_label(*role))
        }
        DomainError::SlotEndsBeforeItStarts => "スロットの開始が終了より前でない".to_string(),
        DomainError::SlotOverlaps { existing } => format!(
            "同じコーチの枠（{}）と期間が重なる",
            slot_id_uuid::format(*existing)
        ),
        DomainError::SlotOfAnotherCoach => "別のコーチの枠は扱えない".to_string(),
        DomainError::SlotNotOpen { status } => {
            format!("スロットが公開中でない（{}）", slot_status_label(*status))
        }
        DomainError::SlotAlreadyBooked => "スロットはすでに予約されている".to_string(),
        DomainError::TrialAlreadyUsed => "体験予約は初回の予約だけ".to_string(),
        DomainError::BookingNotPending { status } => {
            format!("確定前の予約でない（{}）", booking_status_label(*status))
        }
        DomainError::BookingOfAnotherStudent => "別の生徒の予約は扱えない".to_string(),
        DomainError::BookingOfAnotherSlot => "予約と別のスロットは扱えない".to_string(),
    }
}

/// スロットの状態の呼び名。
const fn slot_status_label(status: SlotStatus) -> &'static str {
    match status {
        SlotStatus::Open => "公開中",
        SlotStatus::Booked => "予約済み",
        SlotStatus::Closed => "閉じている",
    }
}

/// 予約の状態の呼び名。
const fn booking_status_label(status: BookingStatus) -> &'static str {
    match status {
        BookingStatus::Pending => "確定前",
        BookingStatus::Confirmed => "確定済み",
        BookingStatus::Cancelled => "キャンセル済み",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_each_domain_error() {
        assert_eq!(domain_error(&DomainError::EmptyDisplayName), "表示名が空");
        assert_eq!(
            domain_error(&DomainError::DisplayNameTooLong { max: 50 }),
            "表示名は 50 文字まで"
        );
        assert_eq!(
            domain_error(&DomainError::InvalidEmail),
            "メールアドレスの形が不正"
        );
        assert_eq!(
            domain_error(&DomainError::RoleAttributeMismatch {
                role: Role::Student,
                sports: 2
            }),
            "生徒にスポーツ 2 個は持てない"
        );
        assert_eq!(
            domain_error(&DomainError::SlotEndsBeforeItStarts),
            "スロットの開始が終了より前でない"
        );
        assert_eq!(
            domain_error(&DomainError::SlotOverlaps {
                existing: domain::scheduling::SlotId::from_u128(1)
            }),
            "同じコーチの枠（00000000-0000-0000-0000-000000000001）と期間が重なる"
        );
        assert_eq!(
            domain_error(&DomainError::SlotOfAnotherCoach),
            "別のコーチの枠は扱えない"
        );
        assert_eq!(
            domain_error(&DomainError::SlotNotOpen {
                status: SlotStatus::Booked
            }),
            "スロットが公開中でない（予約済み）"
        );
        assert_eq!(
            domain_error(&DomainError::SlotAlreadyBooked),
            "スロットはすでに予約されている"
        );
        assert_eq!(
            domain_error(&DomainError::TrialAlreadyUsed),
            "体験予約は初回の予約だけ"
        );
        assert_eq!(
            domain_error(&DomainError::BookingNotPending {
                status: BookingStatus::Confirmed
            }),
            "確定前の予約でない（確定済み）"
        );
        assert_eq!(
            domain_error(&DomainError::BookingOfAnotherStudent),
            "別の生徒の予約は扱えない"
        );
        assert_eq!(
            domain_error(&DomainError::BookingOfAnotherSlot),
            "予約と別のスロットは扱えない"
        );
    }
}
