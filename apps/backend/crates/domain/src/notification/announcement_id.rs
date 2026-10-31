/// 本部からのお知らせ（[`super::Announcement`]）の型付き ID（128 ビットの値）。
///
/// テキスト形式の読み書きは境界のアダプタが行う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnnouncementId(u128);

impl AnnouncementId {
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_value() {
        let value = 0x0123_4567_89ab_4def_8123_4567_89ab_cdef;
        assert_eq!(AnnouncementId::from_u128(value).as_u128(), value);
    }
}
