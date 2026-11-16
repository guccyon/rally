//! `AccessTokenBackend` の選び方（`ACCESS_TOKEN_SIGNING_KEY`・`ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK`。Issue #1593・#1785）。

use application::identity::{InvalidToken, TokenClaims, TokenIssuer, TokenVerifier};
use domain::identity::{AccountId, Role};
use infra::{AccessTokenBackend, AccessTokenConfigError, InvalidSigningKey, MockTokenService};

/// テストだけの署名の鍵（秘密ではない）。
const TEST_KEY: &str = "infra-test-signing-key-0123456789abcdef";

const CLAIMS: TokenClaims = TokenClaims {
    account_id: AccountId::from_u128(42),
    role: Role::Headquarters,
};

fn with(
    key: Option<&str>,
    opt_in: Option<&str>,
) -> Result<AccessTokenBackend, AccessTokenConfigError> {
    AccessTokenBackend::from_vars(|name| match name {
        AccessTokenBackend::SIGNING_KEY_VAR => key.map(str::to_string),
        AccessTokenBackend::UNSIGNED_MOCK_OPT_IN_VAR => opt_in.map(str::to_string),
        other => panic!("知らない環境変数を読んだ: {other}"),
    })
}

#[test]
fn without_a_key_or_the_opt_in_it_is_an_error() {
    for key in [None, Some("")] {
        for opt_in in [None, Some("")] {
            let error = with(key, opt_in).unwrap_err();
            assert_eq!(
                error,
                AccessTokenConfigError::MissingSigningKey,
                "{key:?} {opt_in:?}"
            );
            let message = error.to_string();
            assert!(message.contains("ACCESS_TOKEN_SIGNING_KEY"), "{message}");
            assert!(
                message.contains("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK=1"),
                "{message}"
            );
        }
    }
}

#[test]
fn without_a_key_the_opt_in_selects_the_unsigned_mock() {
    for key in [None, Some("")] {
        let tokens = with(key, Some("1")).unwrap();
        assert!(!tokens.is_signed(), "{key:?}");
        let mock = MockTokenService.issue(CLAIMS);
        assert_eq!(tokens.issue(CLAIMS), mock);
        assert_eq!(tokens.verify(mock.as_str()), Ok(CLAIMS));
    }
}

#[test]
fn an_opt_in_other_than_1_is_an_error() {
    for value in ["0", "true", "yes", " 1"] {
        let error = with(None, Some(value)).unwrap_err();
        assert_eq!(
            error,
            AccessTokenConfigError::InvalidMockOptIn {
                value: value.to_string()
            }
        );
        assert!(
            error
                .to_string()
                .contains("ACCESS_TOKEN_ALLOW_UNSIGNED_MOCK"),
            "{error}"
        );
    }
}

#[test]
fn with_a_key_it_signs_and_rejects_mock_tokens() {
    let tokens = with(Some(TEST_KEY), None).unwrap();
    assert!(tokens.is_signed());
    let token = tokens.issue(CLAIMS);
    assert!(!token.as_str().starts_with("mock."), "{token:?}");
    assert_eq!(tokens.verify(token.as_str()), Ok(CLAIMS));
    // 同じ鍵で起動した別のインスタンス（再起動・staging の access-token）も検証に通す
    assert_eq!(
        with(Some(TEST_KEY), None).unwrap().verify(token.as_str()),
        Ok(CLAIMS)
    );
    let mock = MockTokenService.issue(CLAIMS);
    assert_eq!(tokens.verify(mock.as_str()), Err(InvalidToken));
}

#[test]
fn the_key_wins_over_the_opt_in() {
    // 鍵があれば opt-in の値は見ない（間違った値でも失敗にせず、署名付きにする）
    for opt_in in ["1", "not-a-flag"] {
        let tokens = with(Some(TEST_KEY), Some(opt_in)).unwrap();
        assert!(tokens.is_signed(), "{opt_in}");
        let mock = MockTokenService.issue(CLAIMS);
        assert_eq!(tokens.verify(mock.as_str()), Err(InvalidToken));
    }
}

#[test]
fn a_short_key_is_an_error_and_does_not_fall_back_to_the_mock() {
    for opt_in in [None, Some("1")] {
        let error = with(Some("too-short"), opt_in).unwrap_err();
        assert_eq!(
            error,
            AccessTokenConfigError::InvalidSigningKey(InvalidSigningKey { length: 9 })
        );
        let message = error.to_string();
        assert!(message.contains("ACCESS_TOKEN_SIGNING_KEY"), "{message}");
        assert!(
            !message.contains("too-short"),
            "鍵の値を出さない: {message}"
        );
    }
}

#[test]
fn the_default_is_the_unsigned_mock_for_test_assemblies() {
    assert!(!AccessTokenBackend::default().is_signed());
}
