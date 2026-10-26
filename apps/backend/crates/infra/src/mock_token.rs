//! アクセストークンの発行・検証のモック（Gen1 の外部連携はすべてモック）。

use application::identity::{
    AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier, account_id_uuid,
};

use crate::identity_values::{role_from_value, role_value};

const PREFIX: &str = "mock";

/// [`TokenIssuer`] と [`TokenVerifier`] のモック。
///
/// **署名なし**の固定形式 `mock.<役割>.<アカウント ID>`（例:
/// `mock.student.01234567-89ab-4def-8123-456789abcdef`）で、誰でも任意のアカウント・役割の
/// トークンを作れる。Gen1 のローカル起動とテストのためだけのもので、**本番では署名付きの
/// 実装に差し替える**。状態を持たないので、別のインスタンスが発行したトークンも検証に通る。
/// 役割は [`crate::identity_values`]、アカウント ID は [`account_id_uuid`] の形。
#[derive(Debug, Clone, Copy, Default)]
pub struct MockTokenService;

impl TokenIssuer for MockTokenService {
    fn issue(&self, claims: TokenClaims) -> AccessToken {
        AccessToken::new(format!(
            "{PREFIX}.{}.{}",
            role_value(claims.role),
            account_id_uuid::format(claims.account_id)
        ))
    }
}

impl TokenVerifier for MockTokenService {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        let mut parts = token.split('.');
        let (Some(PREFIX), Some(role), Some(account_id), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(InvalidToken);
        };
        let role = role_from_value(role).ok_or(InvalidToken)?;
        let account_id = account_id_uuid::parse(account_id).map_err(|_| InvalidToken)?;
        Ok(TokenClaims { account_id, role })
    }
}

#[cfg(test)]
mod tests {
    use domain::identity::{AccountId, Role};

    use super::*;

    const ID: AccountId = AccountId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);

    #[test]
    fn verifies_what_it_issued_for_every_role() {
        for role in Role::ALL {
            let claims = TokenClaims {
                account_id: ID,
                role,
            };
            let token = MockTokenService.issue(claims);
            assert_eq!(MockTokenService.verify(token.as_str()), Ok(claims));
        }
    }

    #[test]
    fn issues_the_documented_format() {
        let token = MockTokenService.issue(TokenClaims {
            account_id: ID,
            role: Role::Coach,
        });
        assert_eq!(
            token.as_str(),
            "mock.coach.01234567-89ab-4def-8123-456789abcdef"
        );
    }

    #[test]
    fn rejects_malformed_tokens() {
        for token in [
            "",
            "mock",
            "mock.student",
            "other.student.01234567-89ab-4def-8123-456789abcdef",
            "mock.admin.01234567-89ab-4def-8123-456789abcdef",
            "mock.Student.01234567-89ab-4def-8123-456789abcdef",
            "mock.student.not-a-uuid",
            "mock.student.01234567-89ab-4def-8123-456789abcdef.extra",
            "mock-01234567-89ab-4def-8123-456789abcdef",
        ] {
            assert_eq!(MockTokenService.verify(token), Err(InvalidToken), "{token}");
        }
    }
}
