//! [`SubscriptionId`] と UUID の変換。境界のアダプタ（HTTP の契約の `format: uuid`・
//! 境界間イベントのペイロード・保存先）はここだけを通し、テキスト形式の書き出しは `uuid` クレートに任せる。

use domain::entitlement::SubscriptionId;
use uuid::Uuid;

pub const fn to_uuid(id: SubscriptionId) -> Uuid {
    Uuid::from_u128(id.as_u128())
}

pub const fn from_uuid(uuid: Uuid) -> SubscriptionId {
    SubscriptionId::from_u128(uuid.as_u128())
}

/// 小文字・ハイフン区切り（`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`）。
pub fn format(id: SubscriptionId) -> String {
    to_uuid(id).hyphenated().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_as_hyphenated_uuid_and_round_trips() {
        let id = SubscriptionId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
        assert_eq!(format(id), "01234567-89ab-4def-8123-456789abcdef");
        assert_eq!(from_uuid(to_uuid(id)), id);
    }
}
