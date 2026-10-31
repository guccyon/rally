use std::collections::BTreeMap;

use domain::{async_lesson::Video, identity::AccountId, profile::DisplayName};

/// 生徒が送った動画の一覧（[`super::ListMyVideos`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyVideoList {
    /// 生徒が送った動画（返却された添削を含む）。送った時刻の新しい順で、同じ時刻は動画 ID の順
    pub videos: Vec<Video>,
    /// `videos` の送り先のコーチの表示名（コーチの ID ごと）。表示名が引けないコーチは含まない
    pub coaches: BTreeMap<AccountId, DisplayName>,
}
