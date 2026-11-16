//! 動画の送付と添削（`POST /videos`・`GET`・`POST /videos/{video_id}/review`）。動画の保存先と AI
//! の一次フィードバックは infra のモック。動画の送付は entitlement に生徒が添削を依頼してよいかを問い合わせ
//! （`EntitlementVideoReviewGate`。体験の使用状況は `EntitlementTrialUsageLookup`、まだ受けていない体験予約の解放は
//! scheduling の `CancelBooking`）、添削の返却は entitlement に着手してよいかを問い合わせる（`EntitlementReviewGate`）。

use application::{
    async_lesson::{GetVideoReview, ReturnVideoReview, SubmitVideo},
    catalog::CatalogLessonMenuLookup,
    entitlement::{
        EntitlementQuotaGate, EntitlementReviewGate, EntitlementTrialUsageLookup,
        EntitlementVideoReviewGate,
    },
    scheduling::CancelBooking,
};
use axum::Router;
use infra::{MockAiFeedbackGenerator, MockVideoStorage};

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::videos_router(
        SubmitVideo::new(
            ports.videos(),
            CatalogLessonMenuLookup::new(ports.lesson_menus()),
            MockVideoStorage,
            MockAiFeedbackGenerator,
            EntitlementVideoReviewGate::new(
                EntitlementTrialUsageLookup::new(
                    ports.entitlements(),
                    ports.completions(),
                    ports.videos(),
                ),
                CancelBooking::new(
                    ports.bookings(),
                    EntitlementQuotaGate::new(ports.entitlements()),
                    ports.analytics(),
                    ports.events(),
                    ports.clock(),
                ),
            ),
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
