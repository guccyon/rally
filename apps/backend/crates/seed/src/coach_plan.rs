use domain::catalog::Sport;

/// 合成するコーチ 1 人。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachPlan {
    /// 提供スポーツ（ゴルフ以外の 5 つに配分する）
    pub sport: Sport,
    /// 添削を返すまでの時間の目安（時間）。返却ごとに 0.5〜1.5 倍のばらつきをつける
    pub review_lead_hours: u64,
}
