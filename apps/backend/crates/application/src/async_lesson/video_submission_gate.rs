use std::future::Future;

use domain::identity::AccountId;

use crate::Unavailable;

/// async-lesson が「この生徒が動画を送ってよいか」を問い合わせるポート（体験は 1 回だけ。ADR 0014）。
///
/// 月額契約の生徒は送り放題。契約の前の生徒は体験（通話か動画添削のどちらか 1 回）として 1 本だけ送れる。
/// 体験を使い終えたか（完了したレッスンか、送った動画があるか）の判定は実装が持ち、async-lesson は答えに従うだけ。
/// 実装は [`crate::entitlement::EntitlementVideoSubmissionGate`]。動画の送付（[`super::SubmitVideo`]）が保存の前に呼ぶ。
pub trait VideoSubmissionGate: Send + Sync {
    /// `student_id` の生徒が、すでに `sent_videos` 本の動画を送っているとき、もう 1 本送ってよいか。
    fn may_submit_video(
        &self,
        student_id: AccountId,
        sent_videos: usize,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
