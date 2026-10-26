use crate::Unavailable;

/// [`super::EntitlementRepository::insert_monthly`] の失敗。
#[derive(Debug)]
pub enum InsertEntitlementError {
    /// 生徒の受講権がすでにある（Gen1 の受講権は生徒ごとに 1 つ）
    AlreadyExists,
    /// 保存先の障害
    Unavailable(Unavailable),
}
