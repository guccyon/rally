//! 契約（OpenAPI）の文字列と domain の値の対応表。HTTP の入出力はこの表だけを通す。
//!
//! アカウント ID（`format: uuid`）のテキスト形式は `application::identity::account_id_uuid`。

use domain::identity::Sport;

/// OpenAPI の `Sport` の値。どのクライアントがどれを受け付けるかは各ハンドラーが決める
/// （student-app はゴルフを含まない）。
pub(crate) const fn sport_to_contract(sport: Sport) -> &'static str {
    match sport {
        Sport::Tennis => "tennis",
        Sport::Golf => "golf",
        Sport::Baseball => "baseball",
        Sport::Soccer => "soccer",
        Sport::Basketball => "basketball",
    }
}

pub(crate) fn sport_from_contract(value: &str) -> Option<Sport> {
    Sport::ALL
        .into_iter()
        .find(|sport| sport_to_contract(*sport) == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_five_blueprint_sports() {
        for sport in Sport::ALL {
            assert_eq!(sport_from_contract(sport_to_contract(sport)), Some(sport));
        }
    }

    #[test]
    fn unknown_sport_is_none() {
        for value in ["cricket", "Tennis", "", " tennis"] {
            assert_eq!(sport_from_contract(value), None, "{value:?}");
        }
    }
}
