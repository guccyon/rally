use std::future::Future;

use super::AiFeedbackRequest;
use crate::Unavailable;

/// AI の一次フィードバック（glossary の ai_feedback）を作る外部の AI のポート。送付した動画について、コーチの添削の
/// 前に返す短い文を作る。
///
/// 動画の送付（[`super::SubmitVideo`]）が動画を保存した直後に同じ処理の中で呼ぶ。失敗しても送付は失敗させない
/// （一次フィードバックは空のまま）。Gen1 の実装は infra のモック（`MockAiFeedbackGenerator`。スポーツごとの決まった
/// 文を返し、外には出ない）と、テスト用の [`crate::in_memory::FixedAiFeedbackGenerator`]。
pub trait AiFeedbackGenerator: Send + Sync {
    /// `request` の動画への一次フィードバックの文を作る。
    fn generate(
        &self,
        request: &AiFeedbackRequest,
    ) -> impl Future<Output = Result<String, Unavailable>> + Send;
}
