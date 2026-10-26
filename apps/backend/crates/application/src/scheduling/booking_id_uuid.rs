//! [`BookingId`] と UUID の変換。境界のアダプタ（HTTP の契約の `format: uuid`・境界間イベントの `booking_id`・
//! 保存先）はここだけを通し、テキスト形式の読み書きは `uuid` クレートに任せる。

use domain::scheduling::BookingId;
use uuid::Uuid;

use crate::uuid_text::parse_hyphenated;

/// テキストが UUID のハイフン区切りの形でない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidBookingId;

pub const fn to_uuid(id: BookingId) -> Uuid {
    Uuid::from_u128(id.as_u128())
}

pub const fn from_uuid(uuid: Uuid) -> BookingId {
    BookingId::from_u128(uuid.as_u128())
}

/// 小文字・ハイフン区切り（`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`）。
pub fn format(id: BookingId) -> String {
    to_uuid(id).hyphenated().to_string()
}

/// ハイフン区切りの UUID を読む（大文字小文字は問わない）。
pub fn parse(text: &str) -> Result<BookingId, InvalidBookingId> {
    parse_hyphenated(text)
        .map(from_uuid)
        .ok_or(InvalidBookingId)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses_as_hyphenated_uuid() {
        let id = BookingId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
        assert_eq!(format(id), "01234567-89ab-4def-8123-456789abcdef");
        assert_eq!(parse("01234567-89AB-4DEF-8123-456789ABCDEF"), Ok(id));
    }

    #[test]
    fn rejects_malformed_ids() {
        for text in ["", "0123456789ab4def8123456789abcdef", "not-a-uuid"] {
            assert_eq!(parse(text), Err(InvalidBookingId), "{text:?}");
        }
    }
}
