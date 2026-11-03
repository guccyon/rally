//! 本部が見る生徒 1 人の利用の様子（`GET /students/{student_id}/activity`）。これからの予約とキャンセルの件数を予約の
//! 保存先から（`SchedulingStudentBookingActivityLookup`）、直近の受けたレッスンを完了記録の保存先から
//! （`LessonStudentLessonHistoryLookup`）、送った動画の数と返却待ちの数を動画の保存先から
//! （`AsyncLessonStudentVideoActivityLookup`）、返信のないまま既定の時間の過ぎたスレッドの数をスレッドの保存先から
//! （`MessagingStudentUnansweredMessageThreadCountLookup`）、その生徒がつけた直近の低い評価の件数と未対応の件数を評価の
//! 保存先から（`ReviewStudentLowRatedReviewCountLookup`）、コーチの表示名をアカウントの保存先から
//! （`ProfileCoachDisplayNameLookup`）読む。

use application::{
    async_lesson::AsyncLessonStudentVideoActivityLookup, identity::GetStudentActivity,
    lesson::LessonStudentLessonHistoryLookup,
    messaging::MessagingStudentUnansweredMessageThreadCountLookup,
    profile::ProfileCoachDisplayNameLookup, review::ReviewStudentLowRatedReviewCountLookup,
    scheduling::SchedulingStudentBookingActivityLookup,
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
            AsyncLessonStudentVideoActivityLookup::new(ports.videos()),
            MessagingStudentUnansweredMessageThreadCountLookup::new(ports.threads()),
            ReviewStudentLowRatedReviewCountLookup::new(ports.reviews()),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
