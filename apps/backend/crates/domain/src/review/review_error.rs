use std::fmt;

/// review（評価）の不変条件の違反。評価の点・一言の生成と、評価を付けるときに返す。
///
/// 表示名など複数のコンテキストが共有する値の違反は [`crate::DomainError`] に置く。利用者に見せる文言と HTTP の
/// 状態は境界（interface）が変種から決める。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewError {
    /// 評価の点が `min`〜`max` の整数でない
    RatingOutOfRange { min: u8, max: u8 },
    /// 評価の一言が空（空白だけを含む）
    EmptyLessonReviewComment,
    /// 評価の一言が長すぎる（`max` は文字数）
    LessonReviewCommentTooLong { max: usize },
    /// 完了したレッスンの生徒でないので、そのレッスンを評価できない
    NotTheLessonStudent,
    /// そのレッスン（完了したレッスン）の評価はすでに付けてある（評価は 1 回）
    LessonAlreadyReviewed,
}

impl fmt::Display for ReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for ReviewError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_debug_form() {
        assert_eq!(
            ReviewError::LessonAlreadyReviewed.to_string(),
            "LessonAlreadyReviewed"
        );
        assert_eq!(
            ReviewError::RatingOutOfRange { min: 1, max: 5 }.to_string(),
            "RatingOutOfRange { min: 1, max: 5 }"
        );
    }
}
