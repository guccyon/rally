use std::{fmt, str::FromStr};

use crate::DomainError;

/// Blueprint のスポーツ 5 種類（specs/product-context.md）。
///
/// student-app が扱うのはゴルフ以外の 4 種類（ゴルフは golf-app の範囲）。
/// どのクライアントでどれを受け付けるかは interface が決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sport {
    Tennis,
    Golf,
    Baseball,
    Soccer,
    Basketball,
}

impl Sport {
    pub const ALL: [Sport; 5] = [
        Sport::Tennis,
        Sport::Golf,
        Sport::Baseball,
        Sport::Soccer,
        Sport::Basketball,
    ];

    /// 契約（OpenAPI の `Sport`）と DB（`identity.accounts.sport`）で使う値。
    pub const fn as_str(self) -> &'static str {
        match self {
            Sport::Tennis => "tennis",
            Sport::Golf => "golf",
            Sport::Baseball => "baseball",
            Sport::Soccer => "soccer",
            Sport::Basketball => "basketball",
        }
    }
}

impl fmt::Display for Sport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Sport {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Sport::ALL
            .into_iter()
            .find(|sport| sport.as_str() == s)
            .ok_or_else(|| DomainError::UnknownSport(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_five_blueprint_sports() {
        for sport in Sport::ALL {
            assert_eq!(sport.as_str().parse(), Ok(sport));
        }
    }

    #[test]
    fn unknown_sport_is_a_domain_error() {
        for s in ["cricket", "Tennis", "", " tennis"] {
            assert_eq!(
                s.parse::<Sport>(),
                Err(DomainError::UnknownSport(s.to_string()))
            );
        }
    }
}
