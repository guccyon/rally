use std::time::SystemTime;

use crate::identity::AccountId;

/// 本部によるお知らせの取り下げ（glossary の announcement_withdrawal）。取り下げた本部のアカウントと時刻。
///
/// これを持つお知らせ（[`super::Announcement`]）は取り下げ済みで、生徒のお知らせの一覧に出さない。取り下げを戻す操作はない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnouncementWithdrawal {
    withdrawn_by: AccountId,
    withdrawn_at: SystemTime,
}

impl AnnouncementWithdrawal {
    pub const fn new(withdrawn_by: AccountId, withdrawn_at: SystemTime) -> Self {
        Self {
            withdrawn_by,
            withdrawn_at,
        }
    }

    /// 取り下げた本部のアカウント。
    pub const fn withdrawn_by(&self) -> AccountId {
        self.withdrawn_by
    }

    /// 取り下げた時刻。
    pub const fn withdrawn_at(&self) -> SystemTime {
        self.withdrawn_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_who_withdrew_it_and_when() {
        let withdrawal =
            AnnouncementWithdrawal::new(AccountId::from_u128(5), SystemTime::UNIX_EPOCH);
        assert_eq!(withdrawal.withdrawn_by(), AccountId::from_u128(5));
        assert_eq!(withdrawal.withdrawn_at(), SystemTime::UNIX_EPOCH);
    }
}
