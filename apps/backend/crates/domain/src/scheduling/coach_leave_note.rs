use super::SchedulingError;

/// コーチのお休みで本部が生徒に添える一言（[`super::CoachLeave`]）。空でもよく（書かなかった）、
/// [`CoachLeaveNote::MAX_CHARS`] 文字まで。書いたとおりに持つ（前後の空白も改行も削らない）。長さの数え方は受付を止める理由
/// （[`super::CoachSuspensionReason`]）と同じ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoachLeaveNote(String);

impl CoachLeaveNote {
    /// 一言の上限（文字数。Unicode のスカラー値で数える。admin-web の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 200;

    pub fn new(value: impl Into<String>) -> Result<Self, SchedulingError> {
        let value = value.into();
        if value.chars().count() > Self::MAX_CHARS {
            return Err(SchedulingError::CoachLeaveNoteTooLong {
                max: Self::MAX_CHARS,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 書かなかった（空の）一言か。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_empty_and_texts_up_to_the_limit() {
        assert!(CoachLeaveNote::new("").unwrap().is_empty());
        assert_eq!(
            CoachLeaveNote::new(" ご迷惑をおかけします。\n来週からは予約できます。 ")
                .unwrap()
                .as_str(),
            " ご迷惑をおかけします。\n来週からは予約できます。 "
        );
        assert!(CoachLeaveNote::new("あ".repeat(CoachLeaveNote::MAX_CHARS)).is_ok());
    }

    #[test]
    fn too_long_note_is_a_scheduling_error() {
        assert_eq!(
            CoachLeaveNote::new("あ".repeat(CoachLeaveNote::MAX_CHARS + 1)),
            Err(SchedulingError::CoachLeaveNoteTooLong {
                max: CoachLeaveNote::MAX_CHARS
            })
        );
    }
}
