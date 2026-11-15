use application::identity::account_id_uuid;
use sqlx::types::Uuid;

/// `access-token` のコマンドライン引数。`--hints`（触れる環境のログインの手がかりを JSON で）か、
/// `--account <アカウント ID>`（そのアカウントのトークンを 1 つ）のどちらか 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessTokenArgs {
    Hints,
    Account(Uuid),
}

impl AccessTokenArgs {
    pub const USAGE: &'static str = "使い方: access-token --hints | access-token --account <アカウント ID>\n\
        環境変数 DATABASE_URL の Postgres にあるアカウントのアクセストークンを、サーバーと同じ実装で発行して標準出力に書く\n\
        （ACCESS_TOKEN_SIGNING_KEY があれば署名付き、なければ署名なしのモック）。\n\
        --hints は本部 1 人とコーチ 3 人の [{\"label\": ..., \"value\": <トークン>}]、--account はそのアカウントの役割のトークン";

    /// プログラム名を除いた引数を読む。
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let args: Vec<String> = args.into_iter().collect();
        match args.as_slice() {
            [flag] if flag == "--hints" => Ok(Self::Hints),
            [flag, id] if flag == "--account" => account_id_uuid::parse(id)
                .map(|id| Self::Account(account_id_uuid::to_uuid(id)))
                .map_err(|_| format!("--account はアカウント ID（UUID）（{id:?}）")),
            [flag] if flag == "--account" => Err("--account に値がない".to_string()),
            _ => Err(format!(
                "引数は --hints か --account <ID> のどちらか 1 つ（{args:?}）"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<AccessTokenArgs, String> {
        AccessTokenArgs::parse(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn reads_hints_or_one_account() {
        assert_eq!(parse(&["--hints"]), Ok(AccessTokenArgs::Hints));
        assert_eq!(
            parse(&["--account", "01234567-89ab-4def-8123-456789abcdef"]),
            Ok(AccessTokenArgs::Account(Uuid::from_u128(
                0x0123_4567_89ab_4def_8123_4567_89ab_cdef
            )))
        );
    }

    #[test]
    fn rejects_anything_else() {
        for args in [
            &[][..],
            &["--account"],
            &["--account", "not-a-uuid"],
            &[
                "--hints",
                "--account",
                "01234567-89ab-4def-8123-456789abcdef",
            ],
            &["--role", "headquarters"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }
}
