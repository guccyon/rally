use super::{AnnouncementTitle, NotificationError};
use crate::NameQuery;

/// お知らせの下書きの題の検索語（本部のお知らせの下書きの一覧の `q`）。題に部分一致する下書きを探す。
///
/// 一致のしかた（前後の空白・大文字と小文字・空白だけのとき）は名前の検索語の共通の規則 [`NameQuery`] のとおりで、
/// 前後の空白を除いたあとに [`AnnouncementTitleQuery::MAX_CHARS`] 文字まで。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementTitleQuery(NameQuery);

impl AnnouncementTitleQuery {
    /// 検索語の上限（前後の空白を除いた文字数）。お知らせの題の上限と同じ。
    pub const MAX_CHARS: usize = AnnouncementTitle::MAX_CHARS;

    /// 前後の空白を除いて空なら（空白だけなら）検索語なしとして `None`。長すぎれば
    /// [`NotificationError::AnnouncementTitleQueryTooLong`]。
    pub fn new(value: &str) -> Result<Option<Self>, NotificationError> {
        Ok(NameQuery::new(
            value,
            Self::MAX_CHARS,
            NotificationError::AnnouncementTitleQueryTooLong {
                max: Self::MAX_CHARS,
            },
        )?
        .map(Self))
    }

    /// 題 `title`（下書きの題。空のこともある）がこの検索語に部分一致するか。
    pub fn matches(&self, title: &str) -> bool {
        self.0.matches(title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(value: &str) -> AnnouncementTitleQuery {
        AnnouncementTitleQuery::new(value).unwrap().unwrap()
    }

    #[test]
    fn blank_query_is_no_query() {
        for s in ["", " ", "\u{3000}\t"] {
            assert_eq!(AnnouncementTitleQuery::new(s), Ok(None));
        }
    }

    #[test]
    fn matches_a_part_of_the_title_by_the_common_rule() {
        assert!(query("メンテナンス").matches("夜間メンテナンスのお知らせ"));
        assert!(query("\u{3000}メンテナンス ").matches("夜間メンテナンスのお知らせ"));
        assert!(!query("夜間 メンテナンス").matches("夜間メンテナンスのお知らせ"));
        assert!(query("APP").matches("App の更新"));
        assert!(!query("ＡＰＰ").matches("App の更新"));
        assert!(!query("めんてなんす").matches("メンテナンス"));
        assert!(query("100%").matches("100% 還元"));
        assert!(!query("1_0").matches("100"));
        assert!(!query("お知らせ").matches(""), "空の題には一致しない");
    }

    #[test]
    fn query_up_to_the_limit_is_accepted_and_longer_is_a_notification_error() {
        let limit = "あ".repeat(AnnouncementTitleQuery::MAX_CHARS);
        assert!(
            AnnouncementTitleQuery::new(&format!(" {limit}\u{3000}"))
                .unwrap()
                .is_some()
        );
        assert_eq!(
            AnnouncementTitleQuery::new(&format!("{limit}あ")),
            Err(NotificationError::AnnouncementTitleQueryTooLong {
                max: AnnouncementTitleQuery::MAX_CHARS
            })
        );
    }
}
