//! [`AccountId`] と UUID の変換。境界のアダプタ（HTTP の契約の `format: uuid`・トークン・
//! 保存先）はここだけを通し、テキスト形式の読み書きは `uuid` クレートに任せる。

use domain::identity::AccountId;
use uuid::Uuid;

use crate::uuid_text::parse_hyphenated;

/// テキストが UUID のハイフン区切りの形でない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidAccountId;

pub const fn to_uuid(id: AccountId) -> Uuid {
    Uuid::from_u128(id.as_u128())
}

pub const fn from_uuid(uuid: Uuid) -> AccountId {
    AccountId::from_u128(uuid.as_u128())
}

/// 小文字・ハイフン区切り（`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`）。
pub fn format(id: AccountId) -> String {
    to_uuid(id).hyphenated().to_string()
}

/// ハイフン区切りの UUID を読む（大文字小文字は問わない）。ハイフンなし・波括弧・`urn:uuid:` の
/// 形は受け付けない。
pub fn parse(text: &str) -> Result<AccountId, InvalidAccountId> {
    parse_hyphenated(text)
        .map(from_uuid)
        .ok_or(InvalidAccountId)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses_as_hyphenated_uuid() {
        let id = AccountId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
        assert_eq!(format(id), "01234567-89ab-4def-8123-456789abcdef");
        assert_eq!(parse("01234567-89AB-4DEF-8123-456789ABCDEF"), Ok(id));
        assert_eq!(parse(&format(id)), Ok(id));
    }

    #[test]
    fn rejects_malformed_ids() {
        for text in [
            "",
            "0123456789ab4def812345678 9abcdef",
            "01234567-89ab-4def-8123-456789abcde",
            "+1234567-89ab-4def-8123-456789abcdef",
            "g1234567-89ab-4def-8123-456789abcdef",
            "0123456789ab4def8123456789abcdef",
            "{01234567-89ab-4def-8123-456789abcdef}",
            "urn:uuid:01234567-89ab-4def-8123-456789abcdef",
        ] {
            assert_eq!(parse(text), Err(InvalidAccountId), "{text:?}");
        }
    }
}
