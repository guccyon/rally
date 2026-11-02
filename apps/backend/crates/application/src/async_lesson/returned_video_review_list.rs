use std::collections::BTreeMap;

use domain::{async_lesson::Video, identity::AccountId, profile::DisplayName};
use time::Date;

/// 本部が見る返した動画添削の記録（[`super::ListReturnedVideoReviews`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnedVideoReviewList {
    /// 期間の始まりの日付（日本時間。含む）。省いたときに決めた日付を含めて、実際に使った日付
    pub from: Date,
    /// 期間の終わりの日付（日本時間。含む）。省いたときに決めた日付を含めて、実際に使った日付
    pub to: Date,
    /// 返却済みの動画（選び方と並べ方は domain の [`domain::async_lesson::ReturnedVideoReviewFilter`]。返却の新しい順）
    pub videos: Vec<Video>,
    /// `videos` の担当のコーチの表示名（コーチの ID ごと）。引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
    /// `videos` の生徒の表示名（生徒の ID ごと）。引けない生徒は含まない
    pub students: BTreeMap<AccountId, DisplayName>,
}
