use std::{fmt, str::FromStr};

use crate::DomainError;

/// アカウントの型付き ID（UUID）。生徒の `student_id`・コーチの `coach_id` もこの値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(u128);

impl AccountId {
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    pub const fn as_u128(self) -> u128 {
        self.0
    }
}

/// 小文字・ハイフン区切りの UUID（`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`）。
impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.0;
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            (v >> 96) as u32,
            (v >> 80) as u16,
            (v >> 64) as u16,
            (v >> 48) as u16,
            v & 0xffff_ffff_ffff,
        )
    }
}

/// ハイフン区切りの UUID を読む（大文字小文字は問わない）。
impl FromStr for AccountId {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || DomainError::InvalidId(s.to_string());
        let groups: Vec<&str> = s.split('-').collect();
        let lengths: Vec<usize> = groups.iter().map(|g| g.len()).collect();
        if lengths != [8, 4, 4, 4, 12] {
            return Err(invalid());
        }
        let hex: String = groups.concat();
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid());
        }
        u128::from_str_radix(&hex, 16)
            .map(Self)
            .map_err(|_| invalid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_and_parses_as_hyphenated_uuid() {
        let id = AccountId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
        assert_eq!(id.to_string(), "01234567-89ab-4def-8123-456789abcdef");
        assert_eq!("01234567-89AB-4DEF-8123-456789ABCDEF".parse(), Ok(id));
    }

    #[test]
    fn rejects_malformed_ids() {
        for s in [
            "",
            "0123456789ab4def812345678 9abcdef",
            "01234567-89ab-4def-8123-456789abcde",
            "+1234567-89ab-4def-8123-456789abcdef",
            "g1234567-89ab-4def-8123-456789abcdef",
        ] {
            assert_eq!(
                s.parse::<AccountId>(),
                Err(DomainError::InvalidId(s.to_string()))
            );
        }
    }
}
