//! ポートのインメモリ実装。テストと、infra の実装（Postgres・モックのトラッキング）を
//! 組み立てるまでのローカル起動に使う。プロセスが終われば消える。

use std::{
    sync::{Arc, Mutex},
    time::SystemTime,
};

use domain::identity::{Account, AccountId};

use crate::{
    analytics::{AnalyticsEvent, AnalyticsSink},
    clock::Clock,
    identity::{AccountRepository, InsertAccountError},
};

/// [`AccountRepository`] のインメモリ実装。ID は UUID v4。複製は同じ中身を共有する。
#[derive(Debug, Clone, Default)]
pub struct InMemoryAccountRepository {
    accounts: Arc<Mutex<Vec<Account>>>,
}

impl InMemoryAccountRepository {
    /// 保存したアカウント（保存した順）。
    pub fn all(&self) -> Vec<Account> {
        self.accounts.lock().unwrap().clone()
    }
}

impl AccountRepository for InMemoryAccountRepository {
    fn next_id(&self) -> AccountId {
        AccountId::from_u128(uuid::Uuid::new_v4().as_u128())
    }

    async fn insert(&self, account: Account) -> Result<(), InsertAccountError> {
        let mut accounts = self.accounts.lock().unwrap();
        let email = account.email().normalized();
        if accounts.iter().any(|a| a.email().normalized() == email) {
            return Err(InsertAccountError::EmailAlreadyRegistered);
        }
        accounts.push(account);
        Ok(())
    }
}

/// 発火したイベントを記録する [`AnalyticsSink`]。複製は同じ記録を共有する。
#[derive(Debug, Clone, Default)]
pub struct RecordingAnalyticsSink {
    events: Arc<Mutex<Vec<AnalyticsEvent>>>,
}

impl RecordingAnalyticsSink {
    /// 発火したイベント（発火した順）。
    pub fn events(&self) -> Vec<AnalyticsEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl AnalyticsSink for RecordingAnalyticsSink {
    async fn emit(&self, event: AnalyticsEvent) {
        self.events.lock().unwrap().push(event);
    }
}

/// 常に同じ時刻を返す [`Clock`]。
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub SystemTime);

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}
