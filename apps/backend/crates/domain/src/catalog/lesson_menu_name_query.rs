use crate::{DomainError, NameQuery};

use super::LessonMenuName;

/// レッスンメニューの名前の検索語（本部のレッスンメニューの横断の一覧の `q`）。名前に部分一致するものを探す。
///
/// 一致のしかた（前後の空白・大文字と小文字・空白だけのとき）は名前の検索語の共通の規則 [`NameQuery`] のとおりで、
/// 前後の空白を除いたあとに [`LessonMenuNameQuery::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonMenuNameQuery(NameQuery);

impl LessonMenuNameQuery {
    /// 検索語の上限（前後の空白を除いた文字数）。レッスンメニューの名前の上限と同じ。
    pub const MAX_CHARS: usize = LessonMenuName::MAX_CHARS;

    /// 前後の空白を除いて空なら（空白だけなら）検索語なしとして `None`。
    pub fn new(value: &str) -> Result<Option<Self>, DomainError> {
        Ok(NameQuery::new(
            value,
            Self::MAX_CHARS,
            DomainError::LessonMenuNameQueryTooLong {
                max: Self::MAX_CHARS,
            },
        )?
        .map(Self))
    }

    /// レッスンメニューの名前 `name` がこの検索語に部分一致するか。
    pub fn matches(&self, name: &LessonMenuName) -> bool {
        self.0.matches(name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(value: &str) -> LessonMenuName {
        LessonMenuName::new(value).unwrap()
    }

    fn query(value: &str) -> LessonMenuNameQuery {
        LessonMenuNameQuery::new(value).unwrap().unwrap()
    }

    #[test]
    fn blank_query_is_no_query() {
        for s in ["", " ", "\u{3000}\t"] {
            assert_eq!(LessonMenuNameQuery::new(s), Ok(None));
        }
    }

    #[test]
    fn matches_a_part_of_the_menu_name_by_the_common_rule() {
        assert!(query("フォア").matches(&name("フォア改善 30 分")));
        assert!(query("\u{3000}改善 30 ").matches(&name("フォア改善 30 分")));
        assert!(!query("改善30").matches(&name("フォア改善 30 分")));
        assert!(query("SERVE").matches(&name("Serve の基礎")));
        assert!(!query("ふぉあ").matches(&name("フォア改善")));
        assert!(!query("ＳＥＲＶＥ").matches(&name("Serve の基礎")));
    }

    #[test]
    fn query_up_to_the_limit_is_accepted_and_longer_is_a_domain_error() {
        let limit = "あ".repeat(LessonMenuNameQuery::MAX_CHARS);
        assert!(
            LessonMenuNameQuery::new(&format!(" {limit} "))
                .unwrap()
                .is_some()
        );
        assert_eq!(
            LessonMenuNameQuery::new(&format!("{limit}あ")),
            Err(DomainError::LessonMenuNameQueryTooLong {
                max: LessonMenuNameQuery::MAX_CHARS
            })
        );
    }
}
