use domain::entitlement::EntitlementError;

use crate::Unavailable;

/// [`super::EntitlementRepository::update_by_student`] の失敗。
#[derive(Debug)]
pub enum UpdateEntitlementError {
    /// 生徒の受講権がない
    NotFound,
    /// 変更が不変条件を満たさない（クロージャが返した違反）
    Rejected(EntitlementError),
    /// 保存先の障害
    Unavailable(Unavailable),
}
