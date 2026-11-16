/// `headquarters-account` のコマンドライン引数。`--email <メールアドレス>` と `--display-name <表示名>` の 2 つ（順は問わない）。
///
/// 値の検証（メールアドレスの形・表示名の長さ）は登録のユースケース（domain）が行う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadquartersAccountArgs {
    pub email: String,
    pub display_name: String,
}

impl HeadquartersAccountArgs {
    pub const USAGE: &'static str = "使い方: headquarters-account --email <メールアドレス> --display-name <表示名>\n\
        環境変数 DATABASE_URL の Postgres に本部のアカウントを 1 つ登録し、アカウント ID を標準出力に書く。\n\
        同じメールアドレス（大文字・小文字を区別しない）の本部のアカウントがあれば何もせず、そのアカウント ID を書く";

    /// プログラム名を除いた引数を読む。
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut email = None;
        let mut display_name = None;
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let slot = match flag.as_str() {
                "--email" => &mut email,
                "--display-name" => &mut display_name,
                _ => return Err(format!("知らない引数 {flag:?}")),
            };
            let value = args.next().ok_or_else(|| format!("{flag} に値がない"))?;
            if slot.replace(value).is_some() {
                return Err(format!("{flag} が 2 回ある"));
            }
        }
        Ok(Self {
            email: email.ok_or("--email がない")?,
            display_name: display_name.ok_or("--display-name がない")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<HeadquartersAccountArgs, String> {
        HeadquartersAccountArgs::parse(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn reads_email_and_display_name_in_any_order() {
        let expected = HeadquartersAccountArgs {
            email: "hq@example.com".to_string(),
            display_name: "本部".to_string(),
        };
        assert_eq!(
            parse(&["--email", "hq@example.com", "--display-name", "本部"]),
            Ok(expected.clone())
        );
        assert_eq!(
            parse(&["--display-name", "本部", "--email", "hq@example.com"]),
            Ok(expected)
        );
    }

    #[test]
    fn rejects_missing_repeated_or_unknown_arguments() {
        for args in [
            &[][..],
            &["--email", "hq@example.com"],
            &["--display-name", "本部"],
            &["--email", "hq@example.com", "--display-name"],
            &[
                "--email",
                "hq@example.com",
                "--email",
                "hq2@example.com",
                "--display-name",
                "本部",
            ],
            &[
                "--email",
                "hq@example.com",
                "--display-name",
                "本部",
                "--role",
            ],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }
}
