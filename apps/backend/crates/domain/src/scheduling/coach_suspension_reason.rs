use super::SchedulingError;

/// 本部がコーチの受付を止めたときに書く理由。空でもよく（書かなかった）、[`CoachSuspensionReason::MAX_CHARS`] 文字まで。
/// 書いたとおりに持つ（前後の空白も削らない）。長さと空白の扱いはレッスンメニューの非公開の理由
/// （[`crate::catalog::HeadquartersUnpublishReason`]）と同じ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoachSuspensionReason(String);

impl CoachSuspensionReason {
    /// 理由の上限（文字数。Unicode のスカラー値で数える。admin-web の契約の `maxLength`）。
    pub const MAX_CHARS: usize = 200;

    pub fn new(value: impl Into<String>) -> Result<Self, SchedulingError> {
        let value = value.into();
        if value.chars().count() > Self::MAX_CHARS {
            return Err(SchedulingError::CoachSuspensionReasonTooLong {
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
    fn accepts_empty_and_texts_up_to_the_limit() {
        assert_eq!(
            CoachSuspensionReason::new("").unwrap(),
            CoachSuspensionReason::default()
        );
        assert_eq!(
            CoachSuspensionReason::new(" 休業の申し出 ")
                .unwrap()
                .as_str(),
            " 休業の申し出 "
        );
        assert!(CoachSuspensionReason::new("あ".repeat(CoachSuspensionReason::MAX_CHARS)).is_ok());
    }

    #[test]
    fn too_long_reason_is_a_scheduling_error() {
        assert_eq!(
            CoachSuspensionReason::new("あ".repeat(CoachSuspensionReason::MAX_CHARS + 1)),
            Err(SchedulingError::CoachSuspensionReasonTooLong {
                max: CoachSuspensionReason::MAX_CHARS
            })
        );
    }
}
