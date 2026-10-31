//! entitlement のユースケースとポート。

pub mod calendar_month;
mod consume_live_lesson_quota;
mod entitlement_chat_gate;
pub mod entitlement_id_uuid;
mod entitlement_quota_gate;
mod entitlement_review_gate;
mod get_entitlement;
mod insert_entitlement_error;
mod list_subscription_plans;
mod live_lesson_quota_subscriber;
mod release_live_lesson_quota;
mod repository;
mod start_subscription;
pub mod subscription_id_uuid;
mod update_entitlement_error;

pub use consume_live_lesson_quota::{
    ConsumeLiveLessonQuota, ConsumeLiveLessonQuotaCommand, ConsumeLiveLessonQuotaError,
};
pub use entitlement_chat_gate::EntitlementChatGate;
pub use entitlement_quota_gate::EntitlementQuotaGate;
pub use entitlement_review_gate::EntitlementReviewGate;
pub use get_entitlement::{GetEntitlement, GetEntitlementError, GetEntitlementQuery};
pub use insert_entitlement_error::InsertEntitlementError;
pub use list_subscription_plans::ListSubscriptionPlans;
pub use live_lesson_quota_subscriber::LiveLessonQuotaSubscriber;
pub use release_live_lesson_quota::{
    ReleaseLiveLessonQuota, ReleaseLiveLessonQuotaCommand, ReleaseLiveLessonQuotaError,
};
pub use repository::EntitlementRepository;
pub use start_subscription::{StartSubscription, StartSubscriptionCommand, StartSubscriptionError};
pub use update_entitlement_error::UpdateEntitlementError;
