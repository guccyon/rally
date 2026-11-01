use crate::{DomainError, NameQuery};

use super::DisplayName;

/// 表示名の検索語（本部の生徒の一覧の `q`）。表示名に部分一致するものを探す。
///
/// 一致のしかた（前後の空白・大文字と小文字・空白だけのとき）は名前の検索語の共通の規則 [`NameQuery`] のとおりで、
/// 前後の空白を除いたあとに [`DisplayNameQuery::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayNameQuery(NameQuery);

impl DisplayNameQuery {
    /// 検索語の上限（前後の空白を除いた文字数）。表示名の上限と同じ。
    pub const MAX_CHARS: usize = DisplayName::MAX_CHARS;

    /// 前後の空白を除いて空なら（空白だけなら）検索語なしとして `None`。
    pub fn new(value: &str) -> Result<Option<Self>, DomainError> {
        Ok(NameQuery::new(
            value,
            Self::MAX_CHARS,
            DomainError::DisplayNameQueryTooLong {
                max: Self::MAX_CHARS,
            },
        )?
        .map(Self))
    }

    /// 比べるときの形（[`NameQuery::folded`]）。保存先が同じ規則で絞り込むのに使う。
    pub fn folded(&self) -> &str {
        self.0.folded()
    }

    /// 表示名 `name` がこの検索語に部分一致するか。
    pub fn matches(&self, name: &DisplayName) -> bool {
        self.0.matches(name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(value: &str) -> DisplayName {
        DisplayName::new(value).unwrap()
    }

    fn query(value: &str) -> DisplayNameQuery {
        DisplayNameQuery::new(value).unwrap().unwrap()
    }

    #[test]
    fn blank_query_is_no_query() {
        for s in ["", " ", "\u{3000}\t"] {
            assert_eq!(DisplayNameQuery::new(s), Ok(None));
        }
    }

    #[test]
    fn matches_a_part_of_the_display_name_by_the_common_rule() {
        let q = query(" YAMA ");
        assert_eq!(q.folded(), "yama");
        assert!(q.matches(&name("Taro Yamada")));
        assert!(!q.matches(&name("ＹＡＭＡＤＡ")));
        assert!(query("ろう").matches(&name("じろう先生")));
        assert!(!query("ろう").matches(&name("はなこ")));
    }

    #[test]
    fn query_up_to_the_limit_is_accepted_and_longer_is_a_domain_error() {
        let limit = "あ".repeat(DisplayNameQuery::MAX_CHARS);
        assert!(
            DisplayNameQuery::new(&format!(" {limit} "))
                .unwrap()
                .is_some()
        );
        assert_eq!(
            DisplayNameQuery::new(&format!("{limit}あ")),
            Err(DomainError::DisplayNameQueryTooLong {
                max: DisplayNameQuery::MAX_CHARS
            })
        );
    }
}
