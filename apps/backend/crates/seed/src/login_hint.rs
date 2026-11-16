use application::identity::{TokenClaims, TokenIssuer, account_id_uuid};
use domain::identity::Role;
use infra::identity_values::role_from_value;
use sqlx::{PgPool, types::Uuid};

/// 触れる環境（staging）のログインの手がかり 1 件（`.factory-config/ship.yaml` の `touch.accounts`）。`value` はその
/// アカウントのアクセストークンで、サーバーと同じ実装（`ACCESS_TOKEN_SIGNING_KEY` があれば署名付き）で発行する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginHint {
    pub label: String,
    pub role: Role,
    pub value: String,
}

impl LoginHint {
    /// 合成シードを書いた DB から、最初に登録した本部 1 人と、コーチ 3 人の手がかりを読む（本部が先）。
    ///
    /// 本部の手がかりは実在の本部のアカウントを指す（お休みなど本部の操作の記録は本部のアカウントへの外部キーを持つ。
    /// Issue #1480）。
    pub async fn load(pool: &PgPool, tokens: &impl TokenIssuer) -> Result<Vec<Self>, sqlx::Error> {
        let rows: Vec<(String, String, Uuid)> = sqlx::query_as(
            "(SELECT '本部: ' || display_name, role, account_id FROM identity.accounts
              WHERE role = 'headquarters' ORDER BY created_at, account_id LIMIT 1)
             UNION ALL
             (SELECT 'コーチ: ' || display_name, role, account_id FROM identity.accounts
              WHERE role = 'coach' ORDER BY created_at, account_id LIMIT 3)",
        )
        .fetch_all(pool)
        .await?;
        rows.into_iter()
            .map(|(label, role, account_id)| {
                let role = role_from_value(&role)
                    .ok_or_else(|| sqlx::Error::Decode(format!("知らない役割 {role:?}").into()))?;
                let token = tokens.issue(TokenClaims {
                    account_id: account_id_uuid::from_uuid(account_id),
                    role,
                });
                Ok(Self {
                    label,
                    role,
                    value: token.into_string(),
                })
            })
            .collect()
    }

    /// 手がかりが本部 1 人とコーチ 3 人そろっているか確かめる。欠けていれば、足りない役割と件数、直し方を `Err` で返す
    /// （本部の手がかりがないまま出荷すると、本部の画面に入れない。Issue #1677）。
    pub fn check_complete(hints: &[Self]) -> Result<(), String> {
        let count = |role| hints.iter().filter(|h| h.role == role).count();
        let missing: Vec<String> = [(Role::Headquarters, 1, "本部"), (Role::Coach, 3, "コーチ")]
            .into_iter()
            .filter_map(|(role, need, name)| {
                let found = count(role);
                (found < need).then(|| format!("{name}が {need} 人必要なのに {found} 人"))
            })
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        Err(format!(
            "ログインの手がかりが足りない: {}。DB に本部とコーチのアカウントがない。staging はシードから入れ直す（apps/backend/README.md の「staging でトークンを作る」）",
            missing.join("、")
        ))
    }

    /// 1 つのアカウントのトークンを、DB にある役割で発行する（登録のないアカウントなら `None`）。
    pub async fn for_account(
        pool: &PgPool,
        tokens: &impl TokenIssuer,
        account_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        let role: Option<String> =
            sqlx::query_scalar("SELECT role FROM identity.accounts WHERE account_id = $1")
                .bind(account_id)
                .fetch_optional(pool)
                .await?;
        role.map(|role| {
            let role = role_from_value(&role)
                .ok_or_else(|| sqlx::Error::Decode(format!("知らない役割 {role:?}").into()))?;
            Ok(tokens
                .issue(TokenClaims {
                    account_id: account_id_uuid::from_uuid(account_id),
                    role,
                })
                .into_string())
        })
        .transpose()
    }

    /// shipper が読む JSON（`{"label": ..., "value": ...}`）。
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "label": self.label, "value": self.value })
    }
}
