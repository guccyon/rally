//! 動画の送付と添削（`POST /videos`・`GET`・`POST /videos/{video_id}/review`）。動画の保存先と AI
//! の一次フィードバックは infra のモック、添削の返却は entitlement
//! に着手してよいかを問い合わせる（`EntitlementReviewGate`）。動画の送付は、体験を使い終えた生徒を断る
//! （`EntitlementVideoSubmissionGate`）。

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    catalog::CatalogLessonMenuLookup,
    entitlement::{EntitlementReviewGate, EntitlementVideoSubmissionGate},
};
use axum::Router;
use infra::{MockAiFeedbackGenerator, MockVideoStorage};

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::videos_router(
        SubmitVideo::new(
            EntitlementVideoSubmissionGate::new(ports.entitlements(), ports.completions()),
            ports.videos(),
            CatalogLessonMenuLookup::new(ports.lesson_menus()),
            MockVideoStorage,
            MockAiFeedbackGenerator,
            ports.analytics(),
            ports.events(),
            ports.clock(),
        ),
        GetVideoReview::new(ports.videos()),
        ReturnVideoReview::new(
            ports.videos(),
            EntitlementReviewGate,
            ports.analytics(),
            ports.events(),
            ports.clock(),
        ),
        ports.tokens(),
    )
}
