use std::collections::BTreeSet;

use domain::{
    catalog::Sport,
    profile::{CoachBio, DisplayName},
};

/// 生徒に見せるコーチの紹介（表示名・自己紹介文・提供スポーツと、見た生徒のお気に入りに入っているか）。[`super::GetCoachProfile`] が返す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachProfile {
    pub display_name: DisplayName,
    /// 書いていなければ空（[`CoachBio::default`]）
    pub bio: CoachBio,
    /// どのクライアントでどのスポーツを見せるかは境界が決める（ゴルフも含む）
    pub sports: BTreeSet<Sport>,
    /// 見た生徒のお気に入りに入っているか（お気に入りは matching の持ち物）
    pub is_favorite: bool,
}
