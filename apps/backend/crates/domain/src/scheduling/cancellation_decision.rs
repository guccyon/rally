/// キャンセル規定（glossary の cancellation_policy）による判定の結果。[`super::CancellationPolicy`] が作る。
///
/// 回数の消費を実行するのは entitlement で、scheduling は判定だけを境界間イベント `BookingCancelled` の
/// `consumes_quota` に載せて渡す（docs/adr/0002 Q2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancellationDecision {
    /// このキャンセルで回数（ビデオ通話の暦月の枠）を消費するか
    pub consumes_quota: bool,
}
