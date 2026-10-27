use std::future::Future;

use domain::identity::AccountId;

use crate::Unavailable;

/// messaging が「この生徒はチャットを送ってよいか」を問い合わせるポート（月額の生徒はチャットが送り放題）。
///
/// 判定の規則は domain の [`domain::entitlement::ContractType::unlimited_video_and_chat`] にあり、messaging は答えに
/// 従うだけで契約種別を知らない。問い合わせは照会だけで、回数を消費しない（送り放題）。実装は
/// [`crate::entitlement::EntitlementChatGate`]。生徒の送信（[`super::SendMessage`]）が送る前に呼ぶ。
pub trait ChatGate: Send + Sync {
    /// 生徒 `student_id` がチャットを送ってよいか。
    fn may_chat(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
