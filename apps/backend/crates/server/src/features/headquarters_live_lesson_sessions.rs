//! 本部が期間とコーチ・生徒で見る通話セッションの記録の一覧（`GET /live-lesson-sessions`）。コーチと生徒の表示名を
//! アカウントの保存先から読んで添える。

use application::{
    live_lesson::ListLiveLessonSessions,
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::headquarters_live_lesson_sessions_router(
        ListLiveLessonSessions::new(
            ports.sessions(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
