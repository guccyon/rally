//! UUID のテキスト形式の読み取り。型付き ID と UUID の変換（`*_id_uuid`）が共有する。

use uuid::Uuid;

/// ハイフン区切りの UUID を読む（大文字小文字は問わない）。ハイフンなし・波括弧・`urn:uuid:` の
/// 形は受け付けない。
pub(crate) fn parse_hyphenated(text: &str) -> Option<Uuid> {
    const HYPHENATED_LEN: usize = 36;
    if text.len() != HYPHENATED_LEN {
        return None;
    }
    Uuid::try_parse(text).ok()
}
