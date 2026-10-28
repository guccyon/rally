use std::collections::BTreeMap;

use domain::{
    async_lesson::Video,
    identity::{AccountId, DisplayName},
};

/// コーチに届いた動画の一覧（[`super::ListCoachVideos`] の結果）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachVideoList {
    /// コーチに送られた動画（返却した添削を含む）。添削待ちを送られた時刻の古い順で先に、その後に返却済みを
    /// 返却した時刻の新しい順。同じ時刻は動画 ID の順
    pub videos: Vec<Video>,
    /// `videos` の生徒の表示名（生徒の ID ごと）。表示名が引けない生徒は含まない
    pub students: BTreeMap<AccountId, DisplayName>,
}
