//! 外部の資源（保存先など）の障害。

use std::{error::Error, fmt};

/// 外部の資源（保存先など）が使えない。原因は [`Error::source`] で辿れる。
///
/// 原因には内部の事情（接続先・SQL など）が載るので、クライアントへの応答に出さない。
#[derive(Debug)]
pub struct Unavailable {
    source: Box<dyn Error + Send + Sync>,
}

impl Unavailable {
    pub fn new(source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unavailable: {}", self.source)
    }
}

impl Error for Unavailable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_source() {
        let error = Unavailable::new("connection refused");
        assert_eq!(error.source().unwrap().to_string(), "connection refused");
    }
}
