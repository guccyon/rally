use std::future::Future;

use domain::{
    entitlement::{Entitlement, EntitlementError, EntitlementId, Subscription, SubscriptionId},
    identity::AccountId,
};

use super::{InsertEntitlementError, UpdateEntitlementError};
use crate::Unavailable;

/// 受講権と月額契約の保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryEntitlementRepository`]（Postgres の `entitlement.*` の実装は、アカウントの保存先を
/// Postgres にするときに足す）。
///
/// Gen1 の生徒のスポーツは 1 つで、受講権は生徒ごとに 1 つ。月額の通話の枠は最後に記録した暦月のもの
/// （今の暦月の枠にするのは domain の [`Entitlement::in_month`]）。不変条件の判定はクロージャ（domain の
/// [`Entitlement`]）だけが行い、保存先は読み取りから保存までを直列にする（並行な消費が互いを見ずに両方
/// 保存されることがないように）。
pub trait EntitlementRepository: Send + Sync {
    /// 新しい受講権の ID を払い出す。
    fn next_entitlement_id(&self) -> EntitlementId;

    /// 新しい月額契約の ID を払い出す。
    fn next_subscription_id(&self) -> SubscriptionId;

    /// `student_id` の生徒の受講権。まだなければ `None`。
    fn find_by_student(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<Option<Entitlement>, Unavailable>> + Send;

    /// 月額契約と、それによる受講権を一緒に保存する。生徒の受講権がすでにあれば
    /// [`InsertEntitlementError::AlreadyExists`] で、どちらも保存しない。
    fn insert_monthly(
        &self,
        subscription: Subscription,
        entitlement: Entitlement,
    ) -> impl Future<Output = Result<(), InsertEntitlementError>> + Send;

    /// `student_id` の生徒の受講権を読み、`update` に渡す。`update` が `Ok` なら保存し、保存した受講権を返す
    /// （`Err` なら何も保存しない）。
    fn update_by_student<F>(
        &self,
        student_id: AccountId,
        update: F,
    ) -> impl Future<Output = Result<Entitlement, UpdateEntitlementError>> + Send
    where
        F: FnOnce(&mut Entitlement) -> Result<(), EntitlementError> + Send;
}
