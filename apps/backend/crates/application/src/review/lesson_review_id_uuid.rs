//! [`LessonReviewId`] と UUID の変換。境界のアダプタ（HTTP の契約の `format: uuid`・保存先・行動イベントの属性）は
//! ここだけを通し、テキスト形式の読み書きは `uuid` クレートに任せる。

use domain::review::LessonReviewId;
use uuid::Uuid;

pub const fn to_uuid(id: LessonReviewId) -> Uuid {
    Uuid::from_u128(id.as_u128())
}

pub const fn from_uuid(uuid: Uuid) -> LessonReviewId {
    LessonReviewId::from_u128(uuid.as_u128())
}

/// 小文字・ハイフン区切り（`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`）。
pub fn format(id: LessonReviewId) -> String {
    to_uuid(id).hyphenated().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_as_hyphenated_uuid_and_round_trips() {
        let id = LessonReviewId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
        assert_eq!(format(id), "01234567-89ab-4def-8123-456789abcdef");
        assert_eq!(from_uuid(to_uuid(id)), id);
    }
}
