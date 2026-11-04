//! 本部が見る、終わっていないまま時間の過ぎた通話セッションの一覧（`GET /live-lesson-sessions/overdue`）。コーチと
//! 生徒の表示名をアカウントの保存先から読んで添え、競技で絞るときはコーチの提供スポーツもそこから読む。

use application::{
    live_lesson::ListOverdueLiveLessonSessions,
    profile::{
        ProfileCoachAccountLookup, ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup,
    },
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::overdue_live_lesson_sessions_router(
        ListOverdueLiveLessonSessions::new(
            ports.sessions(),
            ProfileCoachAccountLookup::new(ports.accounts()),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
