//! 本部が見る返した動画添削の記録（`GET /video-reviews/returned`）。担当のコーチと生徒の表示名をアカウントの
//! 保存先から読んで添える。

use application::{
    async_lesson::ListReturnedVideoReviews,
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::returned_video_reviews_router(
        ListReturnedVideoReviews::new(
            ports.videos(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
