//! 利用者に見せる文言（API の `message`）。domain の値やエラーの変種から組み立てる。

use domain::{DomainError, identity::Role};

/// 役割の呼び名。
pub(crate) const fn role_label(role: Role) -> &'static str {
    match role {
        Role::Student => "生徒",
        Role::Coach => "コーチ",
        Role::Headquarters => "本部",
    }
}

/// 不変条件の違反の説明。
pub(crate) fn domain_error(error: &DomainError) -> String {
    match error {
        DomainError::EmptyDisplayName => "表示名が空".to_string(),
        DomainError::DisplayNameTooLong { max } => format!("表示名は {max} 文字まで"),
        DomainError::InvalidEmail => "メールアドレスの形が不正".to_string(),
        DomainError::RoleAttributeMismatch { role, sports } => {
            format!("{}にスポーツ {sports} 個は持てない", role_label(*role))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_each_domain_error() {
        assert_eq!(domain_error(&DomainError::EmptyDisplayName), "表示名が空");
        assert_eq!(
            domain_error(&DomainError::DisplayNameTooLong { max: 50 }),
            "表示名は 50 文字まで"
        );
        assert_eq!(
            domain_error(&DomainError::InvalidEmail),
            "メールアドレスの形が不正"
        );
        assert_eq!(
            domain_error(&DomainError::RoleAttributeMismatch {
                role: Role::Student,
                sports: 2
            }),
            "生徒にスポーツ 2 個は持てない"
        );
    }
}
