//! AI の一次フィードバックのモック（Gen1 の外部連携はすべてモック）。

use application::{
    Unavailable,
    async_lesson::{AiFeedbackGenerator, AiFeedbackRequest},
};
use domain::catalog::Sport;

/// [`AiFeedbackGenerator`] のモック。本物の AI は呼ばず（外に出ない）、動画のスポーツごとの決まった文
/// （[`MockAiFeedbackGenerator::comment_for`]）を返す。動画の ID・形式・生徒の一言は読まない。
#[derive(Debug, Clone, Copy, Default)]
pub struct MockAiFeedbackGenerator;

impl MockAiFeedbackGenerator {
    /// スポーツ `sport` の動画に返す決まった文。
    pub const fn comment_for(sport: Sport) -> &'static str {
        match sport {
            Sport::Tennis => {
                "テイクバックからインパクトまでの流れを確認しました。打点の位置をコーチと一緒に見直しましょう。"
            }
            Sport::Golf => {
                "アドレスからフィニッシュまでのスイングを確認しました。体重移動をコーチと一緒に見直しましょう。"
            }
            Sport::Baseball => {
                "投球・打撃のフォームを確認しました。下半身の使い方をコーチと一緒に見直しましょう。"
            }
            Sport::Soccer => {
                "ボールタッチと体の向きを確認しました。軸足の置き方をコーチと一緒に見直しましょう。"
            }
            Sport::Basketball => {
                "シュートフォームを確認しました。リリースの高さと膝の使い方をコーチと一緒に見直しましょう。"
            }
            Sport::Swimming => {
                "ストロークとキックを確認しました。呼吸のタイミングをコーチと一緒に見直しましょう。"
            }
        }
    }
}

impl AiFeedbackGenerator for MockAiFeedbackGenerator {
    async fn generate(&self, request: &AiFeedbackRequest) -> Result<String, Unavailable> {
        Ok(Self::comment_for(request.sport).to_string())
    }
}
