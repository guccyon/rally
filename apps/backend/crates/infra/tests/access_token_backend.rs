//! `AccessTokenBackend` の選び方（`ACCESS_TOKEN_SIGNING_KEY`。Issue #1593）。

use application::identity::{InvalidToken, TokenClaims, TokenIssuer, TokenVerifier};
use domain::identity::{AccountId, Role};
use infra::{AccessTokenBackend, InvalidSigningKey, MockTokenService};

/// テストだけの署名の鍵（秘密ではない）。
const TEST_KEY: &str = "infra-test-signing-key-0123456789abcdef";

const CLAIMS: TokenClaims = TokenClaims {
    account_id: AccountId::from_u128(42),
    role: Role::Headquarters,
};

fn with_key(key: Option<&str>) -> Result<AccessTokenBackend, InvalidSigningKey> {
    AccessTokenBackend::from_vars(|name| {
        assert_eq!(name, AccessTokenBackend::SIGNING_KEY_VAR);
        key.map(str::to_string)
    })
}

#[test]
fn without_a_key_it_is_the_unsigned_mock() {
    for key in [None, Some("")] {
        let tokens = with_key(key).unwrap();
        assert!(!tokens.is_signed(), "{key:?}");
        let mock = MockTokenService.issue(CLAIMS);
        assert_eq!(tokens.issue(CLAIMS), mock);
        assert_eq!(tokens.verify(mock.as_str()), Ok(CLAIMS));
    }
    assert!(!AccessTokenBackend::default().is_signed());
}

#[test]
fn with_a_key_it_signs_and_rejects_mock_tokens() {
    let tokens = with_key(Some(TEST_KEY)).unwrap();
    assert!(tokens.is_signed());
    let token = tokens.issue(CLAIMS);
    assert!(!token.as_str().starts_with("mock."), "{token:?}");
    assert_eq!(tokens.verify(token.as_str()), Ok(CLAIMS));
    // 同じ鍵で起動した別のインスタンス（再起動・staging の access-token）も検証に通す
    assert_eq!(
        with_key(Some(TEST_KEY)).unwrap().verify(token.as_str()),
        Ok(CLAIMS)
    );
    let mock = MockTokenService.issue(CLAIMS);
    assert_eq!(tokens.verify(mock.as_str()), Err(InvalidToken));
}

#[test]
fn a_short_key_is_an_error_and_does_not_fall_back_to_the_mock() {
    let error = with_key(Some("too-short")).unwrap_err();
    assert_eq!(error, InvalidSigningKey { length: 9 });
    let message = error.to_string();
    assert!(message.contains("ACCESS_TOKEN_SIGNING_KEY"), "{message}");
    assert!(
        !message.contains("too-short"),
        "鍵の値を出さない: {message}"
    );
}
