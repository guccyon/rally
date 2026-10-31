use std::fmt;

/// matching（マッチング）の不変条件の違反。お気に入りに入れるときに返す。
///
/// 利用者に見せる文言と HTTP の状態は境界（interface）が変種から決める。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchingError {
    /// お気に入りに入れようとしたアカウントが生徒でない（お気に入りを持てるのは生徒だけ）
    NotAStudent,
    /// お気に入りに入れようとした相手がコーチでない
    NotACoach,
}

impl fmt::Display for MatchingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for MatchingError {}
