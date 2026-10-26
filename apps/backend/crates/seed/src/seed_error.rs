use std::fmt;

/// 合成シードの失敗。どの生徒のどの出来事で、ユースケースが何を返したか。
#[derive(Debug)]
pub struct SeedError(String);

impl SeedError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SeedError {}
