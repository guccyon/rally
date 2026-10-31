use super::NotificationError;

/// 本部からのお知らせの本文。空（空白だけ）でなく、[`AnnouncementBody::MAX_CHARS`] 文字まで。書いたとおりに持つ
/// （改行も前後の空白もそのまま）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementBody(String);

impl AnnouncementBody {
    /// 本文の上限（文字数。Unicode のスカラー値で数える。契約の `maxLength`）。
    pub const MAX_CHARS: usize = 2000;

    pub fn new(value: impl Into<String>) -> Result<Self, NotificationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(NotificationError::EmptyAnnouncementBody);
        }
        if value.chars().count() > Self::MAX_CHARS {
            return Err(NotificationError::AnnouncementBodyTooLong {
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
    fn accepts_bodies_from_one_char_up_to_the_limit() {
        assert_eq!(AnnouncementBody::new("あ").unwrap().as_str(), "あ");
        assert_eq!(
            AnnouncementBody::new("1 行目\n2 行目\n").unwrap().as_str(),
            "1 行目\n2 行目\n",
            "改行もそのまま"
        );
        assert!(AnnouncementBody::new("あ".repeat(AnnouncementBody::MAX_CHARS)).is_ok());
    }

    #[test]
    fn empty_body_is_an_error() {
        for s in ["", " ", "\u{3000}\n"] {
            assert_eq!(
                AnnouncementBody::new(s),
                Err(NotificationError::EmptyAnnouncementBody)
            );
        }
    }

    #[test]
    fn too_long_body_is_an_error() {
        assert_eq!(
            AnnouncementBody::new("あ".repeat(AnnouncementBody::MAX_CHARS + 1)),
            Err(NotificationError::AnnouncementBodyTooLong {
                max: AnnouncementBody::MAX_CHARS
            })
        );
    }
}
