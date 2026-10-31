use domain::entitlement::SubscriptionPlan;

/// 契約プランの一覧のユースケース（student-app の `listSubscriptionPlans`）。生徒が契約の前に各プランのビデオ通話の
/// 暦月の上限を知るための読み取りで、何も保存せず、スポーツ（ゴルフかどうか）でも変わらない。
///
/// 並びは [`SubscriptionPlan::ALL`] の順（standard・plus）。上限は呼び出し側が
/// [`SubscriptionPlan::live_lessons_per_month`] で読む（不変条件 1 の数字はそこだけに置く）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ListSubscriptionPlans;

impl ListSubscriptionPlans {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self) -> Vec<SubscriptionPlan> {
        SubscriptionPlan::ALL.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_standard_then_plus() {
        assert_eq!(
            ListSubscriptionPlans::new().execute(),
            vec![SubscriptionPlan::Standard, SubscriptionPlan::Plus]
        );
    }

    #[test]
    fn each_plan_carries_its_monthly_live_lesson_limit() {
        let limits: Vec<u8> = ListSubscriptionPlans::new()
            .execute()
            .into_iter()
            .map(SubscriptionPlan::live_lessons_per_month)
            .collect();
        assert_eq!(limits, vec![2, 4], "不変条件 1（standard 2 回・plus 4 回）");
    }
}
