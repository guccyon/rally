/// 月額契約の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscriptionStatus {
    /// 契約中
    Active,
    /// 解約済み
    Cancelled,
}
