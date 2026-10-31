//! entitlement（受講権と契約）の失敗を、OpenAPI の `Error` の応答に写す。
//!
//! 不変条件の違反（[`EntitlementError`]）と、ユースケースの失敗（受講権がない・すでにある）の文言と HTTP の状態・
//! `code` はここだけで決める（`/subscriptions`・`/entitlements` と予約の確定のハンドラーは変種を個別に拾わない）。
//!
//! - ビデオ通話の暦月の枠が残っていない（`LiveLessonQuotaExceeded`）は 409 `live_lesson_quota_exceeded`
//!   （不変条件 1。scheduling が予約の確定で断るときも同じ応答。[`live_lesson_quota_exceeded`]）
//! - 受講権がすでにある（月額契約の 2 回目の開始）は 409 `subscription_already_active`（[`already_subscribed`]）
//! - 受講権がない（月額契約の前の照会）は 404 `not_found`（[`entitlement_not_found`]）
//! - 契約種別とスポーツが合わない（`SportNotCoveredByContract`）・暦月や使った回数の違反（`InvalidMonth`・
//!   `LiveLessonsUsedOverLimit`）は 422 `validation_failed`

use domain::entitlement::{ContractType, EntitlementError};

use crate::{contract::sport_to_contract, error::ApiError};

/// ビデオ通話の暦月の枠が残っていないときの `message`（[`crate::message::domain_error`] も使う）。
pub(crate) const LIVE_LESSON_QUOTA_EXCEEDED: &str = "今月のビデオ通話の枠を使い切っている";

/// ビデオ通話の暦月の枠が残っていない 409（受講権の枠の消費と、scheduling の予約の確定の拒否で同じ）。
pub(crate) fn live_lesson_quota_exceeded() -> ApiError {
    ApiError::conflict("live_lesson_quota_exceeded", LIVE_LESSON_QUOTA_EXCEEDED)
}

/// 生徒の受講権がすでにある（Gen1 の受講権は生徒ごとに 1 つ）409。
pub(crate) fn already_subscribed() -> ApiError {
    ApiError::conflict(
        "subscription_already_active",
        "受講権がすでにある（月額契約は生徒ごとに 1 つ）",
    )
}

/// 生徒の受講権がまだない 404。
pub(crate) fn entitlement_not_found() -> ApiError {
    ApiError::not_found("受講権がない")
}

impl From<EntitlementError> for ApiError {
    fn from(error: EntitlementError) -> Self {
        match error {
            EntitlementError::LiveLessonQuotaExceeded => live_lesson_quota_exceeded(),
            EntitlementError::SportNotCoveredByContract {
                contract_type,
                sport,
            } => Self::validation_failed(format!(
                "{}ではスポーツ {} の契約を結べない",
                contract_type_label(contract_type),
                sport_to_contract(sport)
            )),
            EntitlementError::InvalidMonth { month } => {
                Self::validation_failed(format!("月は 1〜12（{month}）"))
            }
            EntitlementError::LiveLessonsUsedOverLimit { used, limit } => Self::validation_failed(
                format!("ビデオ通話の枠は暦月 {limit} 回まで（使った回数 {used}）"),
            ),
        }
    }
}

/// 契約種別の呼び名。
const fn contract_type_label(contract_type: ContractType) -> &'static str {
    match contract_type {
        ContractType::Monthly => "月額",
        ContractType::OneShot => "ワンショット",
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use domain::catalog::Sport;

    use super::*;

    #[test]
    fn maps_each_entitlement_error_to_a_status_and_code() {
        for (error, status, code, message) in [
            (
                EntitlementError::LiveLessonQuotaExceeded,
                StatusCode::CONFLICT,
                "live_lesson_quota_exceeded",
                "今月のビデオ通話の枠を使い切っている",
            ),
            (
                EntitlementError::SportNotCoveredByContract {
                    contract_type: ContractType::Monthly,
                    sport: Sport::Golf,
                },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "月額ではスポーツ golf の契約を結べない",
            ),
            (
                EntitlementError::SportNotCoveredByContract {
                    contract_type: ContractType::OneShot,
                    sport: Sport::Tennis,
                },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "ワンショットではスポーツ tennis の契約を結べない",
            ),
            (
                EntitlementError::InvalidMonth { month: 13 },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "月は 1〜12（13）",
            ),
            (
                EntitlementError::LiveLessonsUsedOverLimit { used: 3, limit: 2 },
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
                "ビデオ通話の枠は暦月 2 回まで（使った回数 3）",
            ),
        ] {
            let api_error = ApiError::from(error.clone());
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                (status, code, message),
                "{error:?}"
            );
        }
    }

    #[test]
    fn maps_the_use_case_failures_to_a_status_and_code() {
        for (api_error, status, code, message) in [
            (
                already_subscribed(),
                StatusCode::CONFLICT,
                "subscription_already_active",
                "受講権がすでにある（月額契約は生徒ごとに 1 つ）",
            ),
            (
                entitlement_not_found(),
                StatusCode::NOT_FOUND,
                "not_found",
                "受講権がない",
            ),
        ] {
            assert_eq!(
                (api_error.status(), api_error.code(), api_error.message()),
                (status, code, message)
            );
        }
    }
}
