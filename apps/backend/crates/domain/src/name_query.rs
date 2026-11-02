use crate::DomainError;

/// 名前の検索語の共通の規則。名前に部分一致するものを探す（本部の一覧の `q`。表示名は
/// [`crate::profile::DisplayNameQuery`]、レッスンメニューの名前は [`crate::catalog::LessonMenuNameQuery`] がこれを包む）。
///
/// 前後の空白（全角の空白を含む）は除いて扱い、除いたあとに空なら検索語なし。除いたあとの文字数の上限は包む型が決める
/// （探す名前の上限と同じ）。大文字・小文字は ASCII の英字（`A`〜`Z` と `a`〜`z`）だけ区別しない。全角の英字・
/// ひらがなとカタカナなどは書いたとおりに比べる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameQuery(String);

impl NameQuery {
    /// 比べる前にそろえる文字（ASCII の大文字）。[`NameQuery::FOLD_TO`] の同じ位置の文字に置き換える。
    /// そろえ方の定義はこの 2 つの表だけで、保存先も同じ表で名前をそろえる（Postgres なら `translate`）。
    pub const FOLD_FROM: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    /// [`NameQuery::FOLD_FROM`] の各文字の置き換え先（ASCII の小文字）。
    pub const FOLD_TO: &str = "abcdefghijklmnopqrstuvwxyz";

    /// 前後の空白を除いて空なら（空白だけなら）検索語なしとして `None`。除いたあとに `max_chars` 文字を超えたら `too_long`。
    pub fn new(
        value: &str,
        max_chars: usize,
        too_long: DomainError,
    ) -> Result<Option<Self>, DomainError> {
        let value = value.trim();
        if value.is_empty() {
            return Ok(None);
        }
        if value.chars().count() > max_chars {
            return Err(too_long);
        }
        Ok(Some(Self(fold(value))))
    }

    /// 比べるときの形（前後の空白を除き、[`NameQuery::FOLD_FROM`] の文字を [`NameQuery::FOLD_TO`] に
    /// そろえたもの）。保存先が同じ規則で絞り込むのに使う。
    pub fn folded(&self) -> &str {
        &self.0
    }

    /// 名前 `name` がこの検索語に部分一致するか。
    pub fn matches(&self, name: &str) -> bool {
        fold(name).contains(&self.0)
    }
}

/// `value` の [`NameQuery::FOLD_FROM`] の文字を、[`NameQuery::FOLD_TO`] の同じ位置の文字に置き換える
/// （SQL の `translate` と同じ）。
fn fold(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            NameQuery::FOLD_FROM
                .chars()
                .position(|from| from == c)
                .and_then(|i| NameQuery::FOLD_TO.chars().nth(i))
                .unwrap_or(c)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: usize = 5;
    const TOO_LONG: DomainError = DomainError::DisplayNameQueryTooLong { max: MAX };

    fn query(value: &str) -> NameQuery {
        NameQuery::new(value, MAX, TOO_LONG).unwrap().unwrap()
    }

    #[test]
    fn blank_query_is_no_query() {
        for s in ["", " ", "\u{3000}\t"] {
            assert_eq!(NameQuery::new(s, MAX, TOO_LONG), Ok(None));
        }
    }

    #[test]
    fn surrounding_spaces_are_trimmed_and_inner_spaces_are_kept() {
        assert_eq!(query("\u{3000} たろう ").folded(), "たろう");
        assert!(query(" 山田 太郎 ").matches("山田 太郎"));
        assert!(!query("山田 太郎").matches("山田太郎"));
    }

    #[test]
    fn matches_a_part_of_the_name() {
        let q = query("ろう");
        assert!(q.matches("たろう"));
        assert!(q.matches("じろう先生"));
        assert!(!q.matches("はなこ"));
    }

    #[test]
    fn ascii_letters_are_case_insensitive_and_others_are_compared_as_written() {
        assert!(query("TARO").matches("Taro Yamada"));
        assert!(query("yama").matches("Taro YAMADA"));
        // 全角の英字・ひらがなとカタカナは同じとみなさない
        assert!(!query("ｔａｒｏ").matches("ＴＡＲＯ"));
        assert!(!query("たろう").matches("タロウ"));
        // % と _ はワイルドカードでなく文字のまま
        assert!(query("_1%").matches("a_1%"));
        assert!(!query("_1%").matches("a11"));
    }

    #[test]
    fn fold_tables_pair_each_ascii_uppercase_letter_with_its_lowercase() {
        assert_eq!(
            NameQuery::FOLD_FROM.chars().count(),
            NameQuery::FOLD_TO.chars().count()
        );
        assert_eq!(fold("Taro ＹＡＭＡＤＡ 山田"), "taro ＹＡＭＡＤＡ 山田");
    }

    #[test]
    fn query_up_to_the_limit_after_trimming_is_accepted_and_longer_is_the_given_error() {
        let limit = "あ".repeat(MAX);
        assert!(
            NameQuery::new(&format!(" {limit}\u{3000}"), MAX, TOO_LONG)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            NameQuery::new(&format!("{limit}あ"), MAX, TOO_LONG),
            Err(TOO_LONG)
        );
    }
}
