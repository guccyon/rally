/// コーチのプロフィールの項目のうち、空のままにできるもの（本部が「足りない」とみなして拾う対象）。外部での文字列表現は
/// 境界のアダプタが決める。
///
/// 表示名（[`super::DisplayName`]）と提供スポーツ（identity の [`crate::identity::Coach`]。1 つ以上）は空にできないので
/// 含めない。どの項目をいつ「足りない」とみなすかは [`super::MissingCoachProfileFields`] の 1 か所で決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoachProfileField {
    /// 自己紹介文（[`super::CoachBio`]）
    Bio,
}

impl CoachProfileField {
    /// すべての項目（判定と一覧の並びはこの順）。
    pub const ALL: [CoachProfileField; 1] = [CoachProfileField::Bio];
}
