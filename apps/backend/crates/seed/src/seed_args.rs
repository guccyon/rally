use std::time::SystemTime;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// コマンドライン引数（`--seed <整数>`・`--now <RFC 3339 の時刻>`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedArgs {
    /// 乱数の種。同じ種（と同じ `now`）なら同じデータを書く
    pub seed: u64,
    /// 履歴の終わり（「今」）。省略すれば OS の時計
    pub now: Option<SystemTime>,
}

impl SeedArgs {
    /// `--seed` を省略したときの種。
    pub const DEFAULT_SEED: u64 = 42;

    pub const USAGE: &'static str = "使い方: seed [--seed <整数>] [--now <RFC 3339 の時刻>]\n\
        環境変数 DATABASE_URL の Postgres（空の DB）に合成データを書く。--seed の既定は 42、--now の既定は今";

    /// プログラム名を除いた引数を読む。知らない引数・値のない引数・読めない値は失敗にする。
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut parsed = Self {
            seed: Self::DEFAULT_SEED,
            now: None,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} に値がない"));
            match arg.as_str() {
                "--seed" => {
                    let value = value()?;
                    parsed.seed = value
                        .parse()
                        .map_err(|_| format!("--seed は 0 以上の整数（{value:?}）"))?;
                }
                "--now" => {
                    let value = value()?;
                    let now = OffsetDateTime::parse(&value, &Rfc3339)
                        .map_err(|_| format!("--now は RFC 3339 の時刻（{value:?}）"))?;
                    parsed.now = Some(now.into());
                }
                other => return Err(format!("知らない引数 {other:?}")),
            }
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn parse(args: &[&str]) -> Result<SeedArgs, String> {
        SeedArgs::parse(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn seed_defaults_to_42_and_now_to_the_clock() {
        assert_eq!(
            parse(&[]),
            Ok(SeedArgs {
                seed: 42,
                now: None
            })
        );
    }

    #[test]
    fn reads_seed_and_now() {
        assert_eq!(
            parse(&["--seed", "7", "--now", "2026-11-01T09:00:00+09:00"]),
            Ok(SeedArgs {
                seed: 7,
                now: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)),
            })
        );
    }

    #[test]
    fn rejects_unknown_missing_and_invalid_values() {
        for args in [
            &["--seed"][..],
            &["--seed", "-1"],
            &["--seed", "x"],
            &["--now", "2026-11-01"],
            &["--verbose"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }
}
