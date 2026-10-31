use super::NotificationError;

/// 本部からのお知らせの題。空（空白だけ）でなく、[`AnnouncementTitle::MAX_CHARS`] 文字まで。書いたとおりに持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementTitle(String);

impl AnnouncementTitle {
    /// 題の上限（文字数。Unicode のスカラー値で数える。契約の `maxLength`）。
    pub const MAX_CHARS: usize = 50;

    pub fn new(value: impl Into<String>) -> Result<Self, NotificationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(NotificationError::EmptyAnnouncementTitle);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(NotificationError::AnnouncementTitleTooLong {
                max: Self::MAX_CHARS,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_titles_from_one_char_up_to_the_limit() {
        assert_eq!(AnnouncementTitle::new("あ").unwrap().as_str(), "あ");
        assert_eq!(
            AnnouncementTitle::new(" メンテナンスのお知らせ ")
                .unwrap()
                .as_str(),
            " メンテナンスのお知らせ ",
            "前後の空白も削らない"
        );
        assert!(AnnouncementTitle::new("あ".repeat(AnnouncementTitle::MAX_CHARS)).is_ok());
    }

    #[test]
    fn empty_title_is_an_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(
                AnnouncementTitle::new(s),
                Err(NotificationError::EmptyAnnouncementTitle)
            );
        }
    }

    #[test]
    fn too_long_title_is_an_error() {
        assert_eq!(
            AnnouncementTitle::new("あ".repeat(AnnouncementTitle::MAX_CHARS + 1)),
            Err(NotificationError::AnnouncementTitleTooLong {
                max: AnnouncementTitle::MAX_CHARS
            })
        );
    }
}
