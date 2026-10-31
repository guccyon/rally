use domain::{identity::AccountId, profile::DisplayName};

/// コーチを探す一覧の候補になるコーチ（ID と表示名）。持ち主は profile で、matching は読むだけ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoachCandidate {
    pub coach_id: AccountId,
    pub display_name: DisplayName,
}
