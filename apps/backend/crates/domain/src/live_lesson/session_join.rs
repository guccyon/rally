use std::time::SystemTime;

use crate::identity::AccountId;

/// 通話セッションの参加者（コーチか、結び付いた予約の生徒）の最初の入室。
///
/// 参加者が通話に入るたびに [`super::LiveLessonSession::join`] で記録し、入り直しても最初の時刻のまま変わらない。
/// 開始した参加者は開始の時刻に入室したものとする（[`super::LiveLessonSession::start`]）。Issue #1654
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionJoin {
    account_id: AccountId,
    joined_at: SystemTime,
}

impl SessionJoin {
    pub const fn new(account_id: AccountId, joined_at: SystemTime) -> Self {
        Self {
            account_id,
            joined_at,
        }
    }

    /// 入室した参加者。
    pub const fn account_id(&self) -> AccountId {
        self.account_id
    }

    /// 最初に入室した時刻。
    pub const fn joined_at(&self) -> SystemTime {
        self.joined_at
    }
}
