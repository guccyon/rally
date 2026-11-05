//! 本部が見る生徒の一覧（`GET /students`）。契約種別と契約プランを受講権の保存先から、
//! 最後に予約した時刻と直近 30 日の予約の数を予約の保存先から読んで添える。コーチで絞るときは、そのコーチを予約したことがある生徒を予約の保存先から読む。
//! 返事を待っているメッセージのある生徒に絞るときは、返信のないまま既定の時間の過ぎたスレッドの数をスレッドの保存先から
//! （`MessagingStudentUnansweredMessageThreadCountLookup`）読む。返却待ちの動画のある生徒に絞るときは、返却待ちの動画を
//! 動画の保存先から（`AsyncLessonStudentVideoActivityLookup`）読む。未対応の低い評価をつけた生徒に絞るときは、直近の評価を
//! 評価の保存先から（`ReviewStudentLowRatedReviewCountLookup`）読む。

use application::{
    async_lesson::AsyncLessonStudentVideoActivityLookup,
    entitlement::EntitlementStudentContractLookup,
    identity::ListStudents,
    messaging::MessagingStudentUnansweredMessageThreadCountLookup,
    review::ReviewStudentLowRatedReviewCountLookup,
    scheduling::{
        SchedulingStudentCoachBookingLookup, SchedulingStudentLastBookingLookup,
        SchedulingStudentRecentBookingCountLookup,
    },
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::students_router(
        ListStudents::new(
            ports.accounts(),
            EntitlementStudentContractLookup::new(ports.entitlements()),
            SchedulingStudentLastBookingLookup::new(ports.bookings()),
            SchedulingStudentCoachBookingLookup::new(ports.bookings()),
            SchedulingStudentRecentBookingCountLookup::new(ports.bookings()),
            MessagingStudentUnansweredMessageThreadCountLookup::new(ports.threads()),
            AsyncLessonStudentVideoActivityLookup::new(ports.videos()),
            ReviewStudentLowRatedReviewCountLookup::new(ports.reviews()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
