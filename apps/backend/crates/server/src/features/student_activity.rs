//! 本部が見る生徒 1 人の利用の様子（`GET /students/{student_id}/activity`）。これからの予約とキャンセルの件数を予約の
//! 保存先から（`SchedulingStudentBookingActivityLookup`）、直近の受けたレッスンを完了記録の保存先から
//! （`LessonStudentLessonHistoryLookup`）、コーチの表示名をアカウントの保存先から（`ProfileCoachDisplayNameLookup`）読む。

use application::{
    identity::GetStudentActivity, lesson::LessonStudentLessonHistoryLookup,
    profile::ProfileCoachDisplayNameLookup, scheduling::SchedulingStudentBookingActivityLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::student_activity_router(
        GetStudentActivity::new(
            ports.accounts(),
            SchedulingStudentBookingActivityLookup::new(ports.bookings()),
            LessonStudentLessonHistoryLookup::new(ports.completions()),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
