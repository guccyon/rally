//! `/students/{student_id}/activity`。admin-web の `GET /students/{student_id}/activity`（`getStudentActivity`。本部が見る
//! 生徒 1 人の利用の様子）。本部だけが呼べる。

use std::sync::Arc;

use application::{
    clock::Clock,
    identity::{
        AccountRepository, GetStudentActivity, GetStudentActivityError,
        StudentActivityCoachDisplayNameLookup, StudentBookingActivityLookup,
        StudentLessonHistoryLookup, TokenVerifier, account_id_uuid,
    },
    lesson::lesson_completion_id_uuid,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use domain::identity::Role;
use serde::Serialize;

use crate::{
    auth::{TokenVerification, require_role},
    bookings::{BookingBody, booking_body},
    contract::{date_time_to_contract, lesson_type_to_contract},
    error::ApiError,
};

/// `/students/{student_id}/activity` のハンドラーが使うユースケース。
pub struct StudentActivityState<R, B, L, D, C> {
    pub get_student_activity: GetStudentActivity<R, B, L, D, C>,
}

/// ハンドラーが共有する [`StudentActivityState`]。
type Shared<R, B, L, D, C> = Arc<StudentActivityState<R, B, L, D, C>>;

/// admin-web の `StudentActivity`。生徒を特定する情報は表示名と ID だけを載せる。
#[derive(Serialize)]
pub struct StudentActivityBody {
    student_id: String,
    display_name: String,
    upcoming_bookings: Vec<StudentActivityBookingBody>,
    recent_lessons: Vec<StudentActivityLessonBody>,
    cancelled_booking_count: u32,
}

/// admin-web の `StudentActivityBooking`。コーチの表示名は引けなければ載せない。
#[derive(Serialize)]
pub struct StudentActivityBookingBody {
    booking: BookingBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
}

/// admin-web の `StudentActivityLesson`。コーチの表示名は引けなければ載せない。
#[derive(Serialize)]
pub struct StudentActivityLessonBody {
    lesson_id: String,
    coach_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    coach_display_name: Option<String>,
    lesson_type: &'static str,
    completed_at: String,
}

/// `GET /students/{student_id}/activity`。本部だけが呼べる（役割は `require_role` の層が確かめる）。`student_id` が
/// UUID でない・生徒のアカウントがないときは 404。
pub async fn get_student_activity<R, B, L, D, C>(
    State(state): State<Shared<R, B, L, D, C>>,
    Path(student_id): Path<String>,
) -> Result<Json<StudentActivityBody>, ApiError>
where
    R: AccountRepository,
    B: StudentBookingActivityLookup,
    L: StudentLessonHistoryLookup,
    D: StudentActivityCoachDisplayNameLookup,
    C: Clock,
{
    let student_id =
        account_id_uuid::parse(&student_id).map_err(|_| ApiError::not_found("生徒がない"))?;
    let activity = state
        .get_student_activity
        .execute(student_id)
        .await
        .map_err(|error| match error {
            GetStudentActivityError::StudentNotFound => ApiError::not_found("生徒がない"),
            GetStudentActivityError::Unavailable(cause) => ApiError::internal(cause),
        })?;
    let coach_display_name = |coach_id| {
        activity
            .coaches
            .get(&coach_id)
            .map(|name| name.as_str().to_string())
    };
    Ok(Json(StudentActivityBody {
        student_id: account_id_uuid::format(activity.student_id),
        display_name: activity.display_name.as_str().to_string(),
        upcoming_bookings: activity
            .upcoming_bookings
            .iter()
            .map(|booking| {
                Ok(StudentActivityBookingBody {
                    booking: booking_body(booking)?,
                    coach_display_name: coach_display_name(booking.coach_id()),
                })
            })
            .collect::<Result<_, ApiError>>()?,
        recent_lessons: activity
            .recent_lessons
            .iter()
            .map(|lesson| {
                Ok(StudentActivityLessonBody {
                    lesson_id: lesson_completion_id_uuid::format(lesson.id()),
                    coach_id: account_id_uuid::format(lesson.coach_id()),
                    coach_display_name: coach_display_name(lesson.coach_id()),
                    lesson_type: lesson_type_to_contract(lesson.lesson_type()),
                    completed_at: date_time_to_contract(lesson.completed_at())?,
                })
            })
            .collect::<Result<_, ApiError>>()?,
        cancelled_booking_count: activity.cancelled_booking_count,
    }))
}

/// 本部が見る生徒 1 人の利用の様子（admin-web の `GET /students/{student_id}/activity`）のルーター。組み立て用の
/// クレートは他の機能のルーターと `merge` する（同じ検証器を渡す）。
///
/// `GET /students/{student_id}/activity` は本部だけ（コーチ・生徒は 403）。
pub fn student_activity_router<R, B, L, D, C, T>(
    get_student_activity: GetStudentActivity<R, B, L, D, C>,
    tokens: T,
) -> Router
where
    R: AccountRepository + 'static,
    B: StudentBookingActivityLookup + 'static,
    L: StudentLessonHistoryLookup + 'static,
    D: StudentActivityCoachDisplayNameLookup + 'static,
    C: Clock + 'static,
    T: TokenVerifier + 'static,
{
    let tokens: Arc<dyn TokenVerifier> = Arc::new(tokens);
    Router::new()
        .route(
            "/students/{student_id}/activity",
            get(self::get_student_activity::<R, B, L, D, C>),
        )
        .route_layer(require_role(&[Role::Headquarters]))
        .with_state(Arc::new(StudentActivityState {
            get_student_activity,
        }))
        .layer(TokenVerification::layer(tokens))
}
