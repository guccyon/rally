use std::future::Future;

use domain::identity::AccountId;

use crate::Unavailable;

/// async-lesson が「この生徒の動画の添削の依頼を受け付けてよいか」を問い合わせるポート（月額の生徒は送り放題、
/// 月額契約の前の生徒は体験として 1 本だけ）。messaging の [`crate::messaging::ChatGate`] と同じ形。
///
/// 判定の規則は domain の [`domain::entitlement::TrialUsage::may_request_video_review`] にあり、async-lesson は答えに
/// 従うだけで受講権も体験も知らない。実装は [`crate::entitlement::EntitlementVideoReviewGate`]。動画の送付
/// （[`super::SubmitVideo`]）が、選んだレッスンメニューを確かめた後・何かを保存する前に呼ぶ。
///
/// 体験として受け付けるときは、まだ受けていない体験予約をこの問い合わせの中で解放する（キャンセル扱い。体験は通話か
/// 動画添削のどちらか 1 回）。断るとき（false）は何も変えない。
pub trait VideoReviewGate: Send + Sync {
    /// 生徒 `student_id` の動画の添削の依頼を受け付けてよいか。
    fn admit_video_review(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
