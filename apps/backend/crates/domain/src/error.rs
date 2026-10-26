use std::fmt;

/// ドメインの不変条件の違反。モデルの生成時に返す。
///
/// 利用者に見せる文言は境界（interface）が変種から組み立てる。[`fmt::Display`] はデバッグ用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// 表示名が空（空白だけを含む）
    EmptyDisplayName,
    /// 表示名が長すぎる（`max` は文字数）
    DisplayNameTooLong { max: usize },
    /// メールアドレスの形が不正
    InvalidEmail,
    /// 役割とスポーツの組み合わせが不正（生徒は 1 つ、コーチは 1 つ以上、本部はなし）
    RoleAttributeMismatch {
        role: crate::identity::Role,
        sports: usize,
    },
    /// スロットの開始が終了より前でない
    SlotEndsBeforeItStarts,
    /// 同じコーチの時間を占める枠（`existing`）と期間が重なる
    SlotOverlaps { existing: crate::scheduling::SlotId },
    /// 別のコーチの枠をコーチのカレンダーに入れようとした
    SlotOfAnotherCoach,
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for DomainError {}
