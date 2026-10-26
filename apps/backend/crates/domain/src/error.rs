use std::fmt;

/// ドメインの不変条件の違反。モデルの生成時に返す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// 表示名が空（空白だけを含む）
    EmptyDisplayName,
    /// 表示名が長すぎる（`max` は文字数）
    DisplayNameTooLong { max: usize },
    /// メールアドレスの形が不正
    InvalidEmail,
    /// Blueprint の 5 種類のどれでもないスポーツ
    UnknownSport(String),
    /// 型付き ID の形が不正（UUID の文字列でない）
    InvalidId(String),
    /// 役割とスポーツの組み合わせが不正（生徒は 1 つ、コーチは 1 つ以上、本部はなし）
    RoleAttributeMismatch {
        role: crate::identity::Role,
        sports: usize,
    },
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDisplayName => write!(f, "表示名が空"),
            Self::DisplayNameTooLong { max } => write!(f, "表示名は {max} 文字まで"),
            Self::InvalidEmail => write!(f, "メールアドレスの形が不正"),
            Self::UnknownSport(s) => write!(f, "未知のスポーツ: {s}"),
            Self::InvalidId(s) => write!(f, "ID の形が不正: {s}"),
            Self::RoleAttributeMismatch { role, sports } => {
                write!(f, "役割 {} にスポーツ {sports} 個は持てない", role.as_str())
            }
        }
    }
}

impl std::error::Error for DomainError {}
