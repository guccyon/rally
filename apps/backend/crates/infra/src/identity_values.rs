//! identity の値と、infra が保存・発行する文字列の対応表。
//!
//! DB の `identity.accounts.role` / `sport`（CHECK 制約の値）と、モックのトークン
//! （[`crate::MockTokenService`]）の役割はこの表だけを通す。CHECK との一致は
//! `tests/schema.rs` が確かめる。

use domain::{catalog::Sport, identity::Role};

/// `identity.accounts.role` の値。
pub const fn role_value(role: Role) -> &'static str {
    match role {
        Role::Student => "student",
        Role::Coach => "coach",
        Role::Headquarters => "headquarters",
    }
}

pub fn role_from_value(value: &str) -> Option<Role> {
    Role::ALL
        .into_iter()
        .find(|role| role_value(*role) == value)
}

/// `identity.accounts.sport` の値（他のスキーマの `sport` 列と、行動イベントの `sport` 属性も同じ値）。
pub const fn sport_value(sport: Sport) -> &'static str {
    match sport {
        Sport::Tennis => "tennis",
        Sport::Golf => "golf",
        Sport::Baseball => "baseball",
        Sport::Soccer => "soccer",
        Sport::Basketball => "basketball",
        Sport::Swimming => "swimming",
    }
}

pub fn sport_from_value(value: &str) -> Option<Sport> {
    Sport::ALL
        .into_iter()
        .find(|sport| sport_value(*sport) == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        for role in Role::ALL {
            assert_eq!(role_from_value(role_value(role)), Some(role));
        }
        for sport in Sport::ALL {
            assert_eq!(sport_from_value(sport_value(sport)), Some(sport));
        }
    }

    #[test]
    fn unknown_values_are_none() {
        for value in ["", "Student", "admin", " coach"] {
            assert_eq!(role_from_value(value), None, "{value:?}");
        }
        for value in ["", "Tennis", "cricket", " tennis"] {
            assert_eq!(sport_from_value(value), None, "{value:?}");
        }
    }
}
