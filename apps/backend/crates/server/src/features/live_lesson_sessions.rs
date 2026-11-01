//! 通話セッションの開始・終了（`POST /live-lesson-sessions`・`/live-lesson-sessions/{session_id}/end`）。開始は
//! scheduling の予約の保存先に予約を問い合わせる（`SchedulingBookingLookup`）。

use application::{
    live_lesson::{EndLiveLessonSession, StartLiveLessonSession},
    scheduling::SchedulingBookingLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::live_lesson_sessions_router(
        StartLiveLessonSession::new(
            ports.sessions(),
            SchedulingBookingLookup::new(ports.bookings()),
            ports.video_call(),
            ports.analytics(),
            ports.clock(),
        ),
        EndLiveLessonSession::new(
            ports.sessions(),
            ports.video_call(),
            ports.analytics(),
            ports.clock(),
        ),
        MockTokenService,
    )
}
