use std::future::Future;

use domain::{entitlement::Entitlement, identity::AccountId};

use crate::Unavailable;

/// 受講権の保存先のポート。実装はテスト・ローカル起動用の [`crate::in_memory::InMemoryEntitlementRepository`]
/// （Postgres の `entitlement.entitlements` / `live_lesson_quotas` の実装は、月額契約の開始と枠の消費（T11）で足す）。
pub trait EntitlementRepository: Send + Sync {
    /// `student_id` の生徒の受講権。まだなければ `None`。
    ///
    /// Gen1 の生徒のスポーツは 1 つで、受講権は生徒ごとに 1 つ。月額の通話の枠は最後に記録した暦月のもの
    /// （今の暦月の枠にするのは domain の [`Entitlement::in_month`]）。
    fn find_by_student(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<Option<Entitlement>, Unavailable>> + Send;
}
