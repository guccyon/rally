use domain::identity::Role;

/// ログインの手がかりが足りない役割ごとの、必要な件数と見つかった件数。文言は境界（`access-token`）で組み立てる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteHints {
    /// 足りない役割ごとの (役割, 必要な件数, 見つかった件数)。
    pub shortfalls: Vec<(Role, usize, usize)>,
}
