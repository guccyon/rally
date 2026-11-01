use std::{collections::BTreeMap, time::SystemTime};

use domain::{async_lesson::Video, identity::AccountId, profile::DisplayName};

/// 本部が見る返却待ちの動画添削の一覧（[`super::ListPendingVideoReviews`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingVideoReviewList {
    /// 照会した時刻（経過時間 [`domain::async_lesson::PendingVideoReviewFilter::elapsed_hours`] の基準）
    pub as_of: SystemTime,
    /// 返却待ちの動画（選び方と並べ方は domain の [`domain::async_lesson::PendingVideoReviewFilter`]。送付の古い順）
    pub videos: Vec<Video>,
    /// `videos` の担当のコーチの表示名（コーチの ID ごと）。引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
    /// `videos` の生徒の表示名（生徒の ID ごと）。引けない生徒は含まない
    pub students: BTreeMap<AccountId, DisplayName>,
}
