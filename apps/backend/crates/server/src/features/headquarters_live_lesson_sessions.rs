//! 本部が期間とコーチ・生徒・競技・レッスンメニューで見る通話セッションの記録の一覧（`GET /live-lesson-sessions`）。コーチと
//! 生徒の表示名をアカウントの保存先から読んで添え、競技で絞るときはコーチの提供スポーツもそこから読む。レッスンメニューで
//! 絞るときは、セッションに結び付いた予約の予約した時点のメニューを予約の保存先から読む。

use application::{
    live_lesson::ListLiveLessonSessions,
    profile::{
        ProfileCoachAccountLookup, ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup,
    },
    scheduling::SchedulingSessionBookingLessonMenuLookup,
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
            SchedulingSessionBookingLessonMenuLookup::new(ports.bookings()),
        ),
        MockTokenService,
    )
}
