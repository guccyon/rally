//! catalog（レッスンメニュー）の失敗の API の応答。本部による非公開に関わる不変条件の違反は 409（`code` は違反ごと）、
//! それ以外の [`DomainError`] は今までどおり 422 `validation_failed`。文言は [`crate::message::domain_error`]。

use domain::DomainError;

use crate::{error::ApiError, message::domain_error};

/// レッスンメニューの操作（`updateMyLessonMenu`・`unpublishLessonMenu`・`republishLessonMenu`）で domain が断った理由の応答。
pub(crate) fn lesson_menu_error(error: DomainError) -> ApiError {
    let code = match error {
        DomainError::LessonMenuUnpublishedByHeadquarters => {
            "lesson_menu_unpublished_by_headquarters"
        }
        DomainError::LessonMenuAlreadyUnpublishedByHeadquarters => {
            "lesson_menu_already_unpublished_by_headquarters"
        }
        DomainError::LessonMenuNotUnpublishedByHeadquarters => {
            "lesson_menu_not_unpublished_by_headquarters"
        }
        other => return ApiError::from(other),
    };
    ApiError::conflict(code, domain_error(&error))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn headquarters_unpublish_violations_are_conflicts() {
        for (error, code) in [
            (
                DomainError::LessonMenuUnpublishedByHeadquarters,
                "lesson_menu_unpublished_by_headquarters",
            ),
            (
                DomainError::LessonMenuAlreadyUnpublishedByHeadquarters,
                "lesson_menu_already_unpublished_by_headquarters",
            ),
            (
                DomainError::LessonMenuNotUnpublishedByHeadquarters,
                "lesson_menu_not_unpublished_by_headquarters",
            ),
        ] {
            let message = domain_error(&error);
            let api = lesson_menu_error(error);
            assert_eq!((api.status(), api.code()), (StatusCode::CONFLICT, code));
            assert_eq!(api.message(), message);
        }
    }

    #[test]
    fn other_violations_stay_validation_failures() {
        let api = lesson_menu_error(DomainError::HeadquartersUnpublishReasonTooLong { max: 200 });
        assert_eq!(
            (api.status(), api.code()),
            (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed")
        );
    }
}
