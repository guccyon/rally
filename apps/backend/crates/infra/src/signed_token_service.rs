//! サーバーだけが作れる、署名付きのアクセストークン（Issue #1593）。

use std::fmt;

use application::identity::{
    AccessToken, InvalidToken, TokenClaims, TokenIssuer, TokenVerifier, account_id_uuid,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::{
    InvalidSigningKey,
    identity_values::{role_from_value, role_value},
};

const PREFIX: &str = "v1";

/// [`TokenIssuer`] と [`TokenVerifier`] の署名付きの実装。
///
/// 形は `v1.<役割>.<アカウント ID>.<署名>`。署名は `v1.<役割>.<アカウント ID>` の HMAC-SHA256 を base64url
/// （パディングなし）にしたもので、鍵はサーバーだけが持つ（環境変数 `ACCESS_TOKEN_SIGNING_KEY`。
/// [`crate::AccessTokenBackend`]）。鍵を知らなければトークンを作れず、役割・アカウント ID を書き換えると署名が
/// 合わなくなって検証に通らない。モックの形（`mock.<役割>.<アカウント ID>`）は通さない。
///
/// 期限と失効は持たない（モックと同じく、発行したトークンは鍵を替えるまで使える）。鍵を替えると、それまでに発行した
/// トークンはすべて通らなくなる。同じ鍵を持つ別のインスタンスが発行したトークンも検証に通る。
#[derive(Clone)]
pub struct SignedTokenService {
    mac: Hmac<Sha256>,
}

impl SignedTokenService {
    /// 鍵の最小の長さ（バイト）。HMAC-SHA256 の出力の長さに揃える。
    pub const MIN_KEY_BYTES: usize = 32;

    /// `key` で署名・検証する。短い鍵は [`InvalidSigningKey`] で断る。
    pub fn new(key: &[u8]) -> Result<Self, InvalidSigningKey> {
        if key.len() < Self::MIN_KEY_BYTES {
            return Err(InvalidSigningKey { length: key.len() });
        }
        let mac = Hmac::<Sha256>::new_from_slice(key)
            .map_err(|_| InvalidSigningKey { length: key.len() })?;
        Ok(Self { mac })
    }

    fn mac_of(&self, payload: &str) -> Hmac<Sha256> {
        let mut mac = self.mac.clone();
        mac.update(payload.as_bytes());
        mac
    }
}

// 鍵を出さない
impl fmt::Debug for SignedTokenService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignedTokenService")
    }
}

impl TokenIssuer for SignedTokenService {
    fn issue(&self, claims: TokenClaims) -> AccessToken {
        let payload = format!(
            "{PREFIX}.{}.{}",
            role_value(claims.role),
            account_id_uuid::format(claims.account_id)
        );
        let signature = URL_SAFE_NO_PAD.encode(self.mac_of(&payload).finalize().into_bytes());
        AccessToken::new(format!("{payload}.{signature}"))
    }
}

impl TokenVerifier for SignedTokenService {
    fn verify(&self, token: &str) -> Result<TokenClaims, InvalidToken> {
        let (payload, signature) = token.rsplit_once('.').ok_or(InvalidToken)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| InvalidToken)?;
        // 中身を読む前に署名を確かめる（比較は時間が一定）
        self.mac_of(payload)
            .verify_slice(&signature)
            .map_err(|_| InvalidToken)?;
        let mut parts = payload.split('.');
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
    use crate::MockTokenService;

    const ID: AccountId = AccountId::from_u128(0x0123_4567_89ab_4def_8123_4567_89ab_cdef);
    const OTHER: AccountId = AccountId::from_u128(0x0fed_cba9_8765_4321_8123_4567_89ab_cdef);
    const KEY: &[u8] = b"test-signing-key-0123456789abcdef-not-a-secret";

    fn service() -> SignedTokenService {
        SignedTokenService::new(KEY).unwrap()
    }

    #[test]
    fn verifies_what_it_issued_for_every_role() {
        for role in Role::ALL {
            let claims = TokenClaims {
                account_id: ID,
                role,
            };
            let token = service().issue(claims);
            assert!(token.as_str().starts_with("v1."), "{}", token.as_str());
            assert_eq!(service().verify(token.as_str()), Ok(claims));
        }
    }

    #[test]
    fn rejects_mock_tokens() {
        for role in Role::ALL {
            let mock = MockTokenService.issue(TokenClaims {
                account_id: ID,
                role,
            });
            assert_eq!(service().verify(mock.as_str()), Err(InvalidToken));
        }
    }

    #[test]
    fn rejects_tampered_tokens() {
        let token = service()
            .issue(TokenClaims {
                account_id: ID,
                role: Role::Student,
            })
            .into_string();
        let signature = token.rsplit_once('.').unwrap().1;
        let other = account_id_uuid::format(OTHER);
        for forged in [
            // 役割・アカウント ID だけを書き換え、署名はそのまま
            format!(
                "v1.headquarters.{}.{signature}",
                account_id_uuid::format(ID)
            ),
            format!("v1.student.{other}.{signature}"),
            // 署名を落とす・壊す
            format!("v1.student.{}", account_id_uuid::format(ID)),
            format!("{token}x"),
            format!("{token}.extra"),
            token.replacen("v1.", "v2.", 1),
            String::new(),
        ] {
            assert_eq!(service().verify(&forged), Err(InvalidToken), "{forged}");
        }
    }

    #[test]
    fn rejects_tokens_signed_with_another_key() {
        let other = SignedTokenService::new(b"another-signing-key-0123456789abcdef").unwrap();
        let token = other.issue(TokenClaims {
            account_id: ID,
            role: Role::Coach,
        });
        assert_eq!(service().verify(token.as_str()), Err(InvalidToken));
    }

    #[test]
    fn refuses_short_keys() {
        assert_eq!(
            SignedTokenService::new(&[7; 31]).unwrap_err(),
            InvalidSigningKey { length: 31 }
        );
        assert!(SignedTokenService::new(&[7; 32]).is_ok());
    }

    #[test]
    fn does_not_print_the_key() {
        assert_eq!(format!("{:?}", service()), "SignedTokenService");
    }
}
