use domain::identity::{CoachBio, DisplayName};

/// スロットの検索で枠に添えるコーチの紹介（表示名と自己紹介文）。持ち主は identity で、scheduling は読むだけ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachIntroduction {
    pub display_name: DisplayName,
    /// 書いていなければ空（[`CoachBio::default`]）
    pub bio: CoachBio,
}
