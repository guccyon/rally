use crate::DomainError;

/// 生徒が動画を送るときに書くコーチへの相談内容（student-app の `submitVideo` の `note`）。動画とともに保存し、
/// 送り先のコーチだけが読む。[`VideoNote::MAX_CHARS`] 文字まで。
///
/// 空（空白だけ）なら相談内容なしとして扱う（[`VideoNote::new`] が `None` を返す）。中身は書いたとおりに持つ
/// （前後の空白を削らない）。
#[derive(Clone, PartialEq, Eq)]
pub struct VideoNote(String);

impl VideoNote {
    /// 相談内容の上限（文字数。student-app の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 1000;

    /// 空（空白だけ）なら相談内容なしとして `None`。[`VideoNote::MAX_CHARS`] 文字を超えれば
    /// [`DomainError::VideoNoteTooLong`]（空白だけでも長さは数える）。
    pub fn new(value: impl Into<String>) -> Result<Option<Self>, DomainError> {
        let value = value.into();
        if value.chars().count() > Self::MAX_CHARS {
            return Err(DomainError::VideoNoteTooLong {
                max: Self::MAX_CHARS,
            });
        }
        if value.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(Self(value)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 本文は生徒がコーチに宛てた相談なので、デバッグの出力（ログ）に出さない。
impl std::fmt::Debug for VideoNote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VideoNote({} chars)", self.0.chars().count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_note_as_written_up_to_the_limit() {
        assert_eq!(
            VideoNote::new(" サーブのトスが安定しない\n")
                .unwrap()
                .unwrap()
                .as_str(),
            " サーブのトスが安定しない\n"
        );
        assert!(
            VideoNote::new("あ".repeat(VideoNote::MAX_CHARS))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn an_empty_note_is_no_note() {
        for s in ["", " ", "\u{3000}\n\t"] {
            assert_eq!(VideoNote::new(s), Ok(None), "{s:?}");
        }
    }

    #[test]
    fn too_long_note_is_a_domain_error() {
        for s in [
            "あ".repeat(VideoNote::MAX_CHARS + 1),
            " ".repeat(VideoNote::MAX_CHARS + 1),
        ] {
            assert_eq!(
                VideoNote::new(s),
                Err(DomainError::VideoNoteTooLong {
                    max: VideoNote::MAX_CHARS
                })
            );
        }
    }

    #[test]
    fn debug_output_does_not_show_the_note() {
        let note = VideoNote::new("サーブのトスが安定しない").unwrap().unwrap();
        assert_eq!(format!("{note:?}"), "VideoNote(12 chars)");
    }
}
