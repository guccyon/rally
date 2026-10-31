use std::time::SystemTime;

use domain::{
    identity::AccountId,
    messaging::{MessageThread, MessagingError},
};

use super::MessageThreadRepository;
use crate::Unavailable;

/// スレッドを開く入力。`at` は開いた時刻（購読したイベントの `occurred_at`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenMessageThreadCommand {
    pub student_id: AccountId,
    pub coach_id: AccountId,
    pub at: SystemTime,
}

#[derive(Debug)]
pub enum OpenMessageThreadError {
    /// スレッドを開けない（生徒とコーチが同じアカウント。[`MessagingError::MessageThreadWithOneself`]）
    Invalid(MessagingError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 生徒と担当コーチのスレッドを開くユースケース。messaging の購読（[`super::MessageThreadSubscriber`]）が、
/// 予約の確定（`BookingConfirmed`）で呼ぶ。
///
/// 同じ生徒とコーチのスレッドがすでにあれば新しく開かず、既存のスレッドを返す（[`MessageThreadRepository::open`]）。
/// 行動イベントは発火しない。
pub struct OpenMessageThread<T> {
    threads: T,
}

impl<T> OpenMessageThread<T>
where
    T: MessageThreadRepository,
{
    pub fn new(threads: T) -> Self {
        Self { threads }
    }

    pub async fn execute(
        &self,
        command: OpenMessageThreadCommand,
    ) -> Result<MessageThread, OpenMessageThreadError> {
        let thread = MessageThread::open(
            self.threads.next_thread_id(),
            command.student_id,
            command.coach_id,
            command.at,
        )
        .map_err(OpenMessageThreadError::Invalid)?;
        self.threads
            .open(thread)
            .await
            .map_err(OpenMessageThreadError::Unavailable)
    }

    pub fn threads(&self) -> &T {
        &self.threads
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::in_memory::InMemoryMessageThreadRepository;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800 + hour * 3600)
    }

    fn command(coach_id: AccountId, hour: u64) -> OpenMessageThreadCommand {
        OpenMessageThreadCommand {
            student_id: STUDENT,
            coach_id,
            at: at(hour),
        }
    }

    #[tokio::test]
    async fn opens_one_thread_per_student_and_coach() {
        let use_case = OpenMessageThread::new(InMemoryMessageThreadRepository::default());
        let first = use_case.execute(command(COACH, 0)).await.unwrap();
        assert_eq!((first.student_id(), first.coach_id()), (STUDENT, COACH));
        assert_eq!(first.opened_at(), at(0));

        let again = use_case.execute(command(COACH, 5)).await.unwrap();
        assert_eq!(again, first, "同じ生徒とコーチは既存のスレッドのまま");

        let other = use_case.execute(command(OTHER_COACH, 5)).await.unwrap();
        assert_ne!(other.id(), first.id());
        assert_eq!(use_case.threads().all().len(), 2);
    }

    #[tokio::test]
    async fn a_thread_with_oneself_is_rejected() {
        let use_case = OpenMessageThread::new(InMemoryMessageThreadRepository::default());
        let result = use_case
            .execute(OpenMessageThreadCommand {
                student_id: STUDENT,
                coach_id: STUDENT,
                at: at(0),
            })
            .await;
        assert!(
            matches!(
                result,
                Err(OpenMessageThreadError::Invalid(
                    MessagingError::MessageThreadWithOneself
                ))
            ),
            "{result:?}"
        );
        assert!(use_case.threads().all().is_empty());
    }
}
