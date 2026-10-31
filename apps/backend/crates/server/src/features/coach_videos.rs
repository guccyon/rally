//! コーチに届いた動画の一覧（`GET /coaches/me/videos`）。
//! 動画を送った生徒の表示名を添える（`ProfileStudentDisplayNameLookup`）。

use application::{async_lesson::ListCoachVideos, profile::ProfileStudentDisplayNameLookup};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coach_videos_router(
        ListCoachVideos::new(
            ports.videos(),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
