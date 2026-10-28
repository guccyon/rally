/// 月額契約のプラン（glossary の subscription_plan）。ビデオ通話の暦月の上限を決める。
///
/// 不変条件 1「暦月の上限は契約プランで決まる（standard 2 回・plus 4 回）。上限を超えては予約できない」の上限は
/// [`Self::live_lessons_per_month`] の 1 か所だけに置く。料金と決済は billing の持ち物（Gen1 の範囲外）で、ここには
/// 持たない。外部での文字列表現（契約の `SubscriptionPlan`・DB の `plan`）は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SubscriptionPlan {
    /// スタンダード。暦月 2 回。契約の開始でプランを選ばなかったときの既定
    #[default]
    Standard,
    /// プラス。暦月 4 回
    Plus,
}

impl SubscriptionPlan {
    /// 全てのプラン。
    pub const ALL: [SubscriptionPlan; 2] = [SubscriptionPlan::Standard, SubscriptionPlan::Plus];

    /// このプランのビデオ通話の暦月の上限（不変条件 1）。
    pub const fn live_lessons_per_month(self) -> u8 {
        match self {
            SubscriptionPlan::Standard => 2,
            SubscriptionPlan::Plus => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_allows_2_and_plus_allows_4_live_lessons_a_month() {
        assert_eq!(SubscriptionPlan::Standard.live_lessons_per_month(), 2);
        assert_eq!(SubscriptionPlan::Plus.live_lessons_per_month(), 4);
    }

    #[test]
    fn default_is_standard() {
        assert_eq!(SubscriptionPlan::default(), SubscriptionPlan::Standard);
    }
}
