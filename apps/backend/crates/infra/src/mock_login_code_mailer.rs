//! ログインの確認コードのメールの送信（SES / SendGrid 相当）のモック（Gen1 の外部連携はすべてモック）。

use std::sync::{Arc, Mutex};

use application::{Unavailable, identity::LoginCodeSender};
use domain::identity::{Email, LoginCode};

/// [`LoginCodeSender`] のモック。実際のメールは送らず、送ったメールアドレスとコードを記録して（[`MockLoginCodeMailer::sent`]）、
/// `login_code <メールアドレス> <コード>` の 1 行で標準出力に書く（触れる環境でコードを確かめる手段。本物の送信に替えたら
/// コードはどこにも書き出さない）。記録はプロセスのメモリにだけ持ち、複製は同じ記録を共有する。
#[derive(Debug, Clone, Default)]
pub struct MockLoginCodeMailer {
    sent: Arc<Mutex<Vec<(Email, LoginCode)>>>,
}

impl MockLoginCodeMailer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 送ったメールアドレスとコード（送った順）。
    pub fn sent(&self) -> Vec<(Email, LoginCode)> {
        self.sent.lock().unwrap().clone()
    }

    /// 標準出力に書く 1 行。
    pub fn line(email: &Email, code: &LoginCode) -> String {
        format!("login_code {} {}", email.as_str(), code.as_str())
    }
}

impl LoginCodeSender for MockLoginCodeMailer {
    async fn send(&self, email: &Email, code: &LoginCode) -> Result<(), Unavailable> {
        println!("{}", Self::line(email, code));
        self.sent
            .lock()
            .unwrap()
            .push((email.clone(), code.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_and_prints_the_code_without_sending() {
        let mailer = MockLoginCodeMailer::new();
        let email = Email::new("Taro@example.com").unwrap();
        let code = LoginCode::new("012345").unwrap();

        mailer.clone().send(&email, &code).await.unwrap();

        assert_eq!(mailer.sent(), vec![(email.clone(), code.clone())]);
        assert_eq!(
            MockLoginCodeMailer::line(&email, &code),
            "login_code Taro@example.com 012345"
        );
    }
}
