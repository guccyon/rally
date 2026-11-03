//! 本部が期間とコーチ・生徒・競技で見る通話セッションの記録の一覧（`GET /live-lesson-sessions`）。コーチと生徒の表示名を
//! アカウントの保存先から読んで添え、競技で絞るときはコーチの提供スポーツもそこから読む。

use application::{
    live_lesson::ListLiveLessonSessions,
    profile::{
        ProfileCoachAccountLookup, ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup,
    },
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::headquarters_live_lesson_sessions_router(
        ListLiveLessonSessions::new(
            ports.sessions(),
            ProfileCoachAccountLookup::new(ports.accounts()),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
