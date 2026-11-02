use crate::DomainError;

use super::DisplayName;

/// 表示名の検索語（本部の生徒の一覧・コーチの一覧の `q`）。表示名に部分一致するものを探す。一致の規則はここだけに置き、
/// どちらの一覧もこれで比べる。
///
/// 前後の空白（全角の空白を含む）は除いて扱い、除いたあとに [`DisplayNameQuery::MAX_CHARS`] 文字まで。大文字・小文字は
/// ASCII の英字（`A`〜`Z` と `a`〜`z`）だけ区別しない。全角の英字・ひらがなとカタカナなどは書いたとおりに比べる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayNameQuery(String);

impl DisplayNameQuery {
    /// 検索語の上限（前後の空白を除いた文字数）。表示名の上限と同じ。
    pub const MAX_CHARS: usize = DisplayName::MAX_CHARS;

    /// 比べる前にそろえる文字（ASCII の大文字）。[`DisplayNameQuery::FOLD_TO`] の同じ位置の文字に置き換える。
    /// そろえ方の定義はこの 2 つの表だけで、保存先も同じ表で表示名をそろえる（Postgres なら `translate`）。
    pub const FOLD_FROM: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    /// [`DisplayNameQuery::FOLD_FROM`] の各文字の置き換え先（ASCII の小文字）。
    pub const FOLD_TO: &str = "abcdefghijklmnopqrstuvwxyz";

    /// 前後の空白を除いて空なら（空白だけなら）検索語なしとして `None`。
    pub fn new(value: &str) -> Result<Option<Self>, DomainError> {
        let value = value.trim();
        if value.is_empty() {
            return Ok(None);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::DisplayNameQueryTooLong {
                max: Self::MAX_CHARS,
            });
        }
        Ok(Some(Self(fold(value))))
    }

    /// 比べるときの形（前後の空白を除き、[`DisplayNameQuery::FOLD_FROM`] の文字を [`DisplayNameQuery::FOLD_TO`] に
    /// そろえたもの）。保存先が同じ規則で絞り込むのに使う。
    pub fn folded(&self) -> &str {
        &self.0
    }

    /// 表示名 `name` がこの検索語に部分一致するか。
    pub fn matches(&self, name: &DisplayName) -> bool {
        fold(name.as_str()).contains(&self.0)
    }
}

/// `value` の [`DisplayNameQuery::FOLD_FROM`] の文字を、[`DisplayNameQuery::FOLD_TO`] の同じ位置の文字に置き換える
/// （SQL の `translate` と同じ）。
fn fold(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            DisplayNameQuery::FOLD_FROM
                .chars()
                .position(|from| from == c)
                .and_then(|i| DisplayNameQuery::FOLD_TO.chars().nth(i))
                .unwrap_or(c)
        })
        .collect()
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
    fn surrounding_spaces_are_trimmed_and_inner_spaces_are_kept() {
        assert_eq!(query("\u{3000} たろう ").folded(), "たろう");
        assert!(query(" 山田 太郎 ").matches(&name("山田 太郎")));
        assert!(!query("山田 太郎").matches(&name("山田太郎")));
    }

    #[test]
    fn matches_a_part_of_the_display_name() {
        let q = query("ろう");
        assert!(q.matches(&name("たろう")));
        assert!(q.matches(&name("じろう先生")));
        assert!(!q.matches(&name("はなこ")));
    }

    #[test]
    fn ascii_letters_are_case_insensitive_and_others_are_compared_as_written() {
        assert!(query("TARO").matches(&name("Taro Yamada")));
        assert!(query("yamada").matches(&name("Taro YAMADA")));
        // 全角の英字・ひらがなとカタカナは同じとみなさない
        assert!(!query("ｔａｒｏ").matches(&name("ＴＡＲＯ")));
        assert!(!query("たろう").matches(&name("タロウ")));
    }

    #[test]
    fn fold_tables_pair_each_ascii_uppercase_letter_with_its_lowercase() {
        assert_eq!(
            DisplayNameQuery::FOLD_FROM.chars().count(),
            DisplayNameQuery::FOLD_TO.chars().count()
        );
        assert_eq!(fold("Taro ＹＡＭＡＤＡ 山田"), "taro ＹＡＭＡＤＡ 山田");
    }

    #[test]
    fn percent_and_underscore_are_compared_as_characters() {
        assert!(query("%").matches(&name("100% コーチ")));
        assert!(!query("%").matches(&name("たろう")));
        assert!(query("a_b").matches(&name("a_b")));
        assert!(!query("a_b").matches(&name("axb")));
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
