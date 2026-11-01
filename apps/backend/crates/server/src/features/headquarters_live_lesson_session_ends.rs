//! 本部によるライブレッスンの終了（`POST /live-lesson-sessions/{session_id}/headquarters-end`）。

use application::live_lesson::EndLiveLessonSessionByHeadquarters;
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::headquarters_live_lesson_session_ends_router(
        EndLiveLessonSessionByHeadquarters::new(ports.sessions(), ports.clock()),
        MockTokenService,
    )
}
