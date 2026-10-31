use std::time::SystemTime;

/// キャンセルする前に生徒に見せる、今キャンセルしたらどうなるかの見込み。キャンセル規定（glossary の
/// cancellation_policy）を当てた結果で、[`super::CancellationPolicy::preview`] だけが作る。
///
/// キャンセルしたときの判定（[`super::CancellationDecision`]）と同じ規則から出すので、見込みと実際の判定は食い違わない
/// （同じ時刻にキャンセルすれば `consumes_quota` は判定と同じ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancellationPreview {
    /// 今キャンセルしたら回数（ビデオ通話の暦月の枠）を使うか
    pub consumes_quota: bool,
    /// この時刻より前にキャンセルすれば回数を使わない境目（開始の 24 時間前）。境目のない予約
    /// （生徒がビデオ通話の暦月の枠を持たず、いつキャンセルしても回数を使わない）は `None`
    pub free_cancel_until: Option<SystemTime>,
}
