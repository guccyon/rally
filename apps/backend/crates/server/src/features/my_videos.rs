//! 生徒が送った動画の一覧（`GET /videos/me`）。
//! 送り先のコーチの表示名をアカウントの保存先から読んで添える（`ProfileCoachDisplayNameLookup`）。

use application::{async_lesson::ListMyVideos, profile::ProfileCoachDisplayNameLookup};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::my_videos_router(
        ListMyVideos::new(
            ports.videos(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
