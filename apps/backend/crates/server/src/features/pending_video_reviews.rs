//! 本部が見る返却待ちの動画添削の一覧（`GET /video-reviews/pending`）。担当のコーチと生徒の表示名をアカウントの
//! 保存先から読んで添える。

use application::{
    async_lesson::ListPendingVideoReviews,
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::pending_video_reviews_router(
        ListPendingVideoReviews::new(
            ports.videos(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
