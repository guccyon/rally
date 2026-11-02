//! [`AnnouncementDraftId`] と UUID の変換。境界のアダプタ（HTTP の契約の `format: uuid`・保存先）は
//! ここだけを通し、テキスト形式の読み書きは `uuid` クレートに任せる。

use domain::notification::AnnouncementDraftId;
use uuid::Uuid;

use crate::uuid_text::parse_hyphenated;

/// テキストが UUID のハイフン区切りの形でない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidAnnouncementDraftId;

pub const fn to_uuid(id: AnnouncementDraftId) -> Uuid {
    Uuid::from_u128(id.as_u128())
}

pub const fn from_uuid(uuid: Uuid) -> AnnouncementDraftId {
    AnnouncementDraftId::from_u128(uuid.as_u128())
}

/// 小文字・ハイフン区切り（`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`）。
pub fn format(id: AnnouncementDraftId) -> String {
    to_uuid(id).hyphenated().to_string()
}

/// ハイフン区切りの UUID を読む（大文字小文字は問わない）。
pub fn parse(text: &str) -> Result<AnnouncementDraftId, InvalidAnnouncementDraftId> {
    parse_hyphenated(text)
        .map(from_uuid)
        .ok_or(InvalidAnnouncementDraftId)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_as_hyphenated_uuid_and_round_trips() {
        let id = AnnouncementDraftId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
        assert_eq!(format(id), "01234567-89ab-4def-8123-456789abcdef");
        assert_eq!(from_uuid(to_uuid(id)), id);
    }

    #[test]
    fn parses_a_hyphenated_uuid_and_rejects_others() {
        let id = AnnouncementDraftId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
        assert_eq!(parse("01234567-89AB-4DEF-8123-456789ABCDEF"), Ok(id));
        assert_eq!(parse("not-a-uuid"), Err(InvalidAnnouncementDraftId));
    }
}
