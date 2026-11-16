use std::fmt;

use crate::DomainError;

/// ログインの確認コード（メールで送る 6 桁の数字。ADR 0012）。
///
/// 先頭の 0 も桁に数える（`"000123"` は 6 桁）。
#[derive(Clone, PartialEq, Eq)]
pub struct LoginCode(String);

impl LoginCode {
    /// 桁数。
    pub const DIGITS: usize = 6;

    /// 桁の種類の数（`10^6`）。[`LoginCode::from_number`] はこの数で割った余りを使う。
    const SPACE: u64 = 1_000_000;

    /// 入力の文字列から作る。ちょうど 6 桁の ASCII の数字でなければ [`DomainError::InvalidLoginCode`]。
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.len() == Self::DIGITS && value.bytes().all(|b| b.is_ascii_digit()) {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidLoginCode)
        }
    }

    /// 乱数から作る（`10^6` で割った余りを 0 埋めの 6 桁にする）。乱数の生成は呼ぶ側（application）が行う。
    pub fn from_number(number: u64) -> Self {
        Self(format!(
            "{:0width$}",
            number % Self::SPACE,
            width = Self::DIGITS
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `other` と同じコードか。比べる時間が一致した桁の数で変わらないように、すべての桁を比べる。
    pub fn matches(&self, other: &Self) -> bool {
        self.0
            .bytes()
            .zip(other.0.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
    }
}

/// デバッグの出力にコードを出さない（ログに確認コードを残さない）。
impl fmt::Debug for LoginCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LoginCode(******)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exactly_six_ascii_digits() {
        for s in ["000000", "123456", "999999", "012345"] {
            assert_eq!(LoginCode::new(s).unwrap().as_str(), s);
        }
    }

    #[test]
    fn anything_else_is_invalid_login_code() {
        for s in [
            "",
            "12345",
            "1234567",
            "12345a",
            " 123456",
            "１２３４５６",
            "12 456",
        ] {
            assert_eq!(
                LoginCode::new(s),
                Err(DomainError::InvalidLoginCode),
                "{s:?}"
            );
        }
    }

    #[test]
    fn from_number_pads_with_zeros_and_wraps_at_a_million() {
        assert_eq!(LoginCode::from_number(42).as_str(), "000042");
        assert_eq!(LoginCode::from_number(123_456).as_str(), "123456");
        assert_eq!(LoginCode::from_number(7_000_001).as_str(), "000001");
        assert_eq!(LoginCode::from_number(u64::MAX).as_str().len(), 6);
    }

    #[test]
    fn matches_only_the_same_code() {
        let code = LoginCode::new("123456").unwrap();
        assert!(code.matches(&LoginCode::new("123456").unwrap()));
        assert!(!code.matches(&LoginCode::new("123457").unwrap()));
        assert!(!code.matches(&LoginCode::new("023456").unwrap()));
    }

    #[test]
    fn debug_hides_the_code() {
        let code = LoginCode::new("123456").unwrap();
        assert_eq!(format!("{code:?}"), "LoginCode(******)");
    }
}
