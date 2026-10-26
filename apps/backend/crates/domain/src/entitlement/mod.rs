//! entitlement: 受講権（specs/context-map.yaml）。
//!
//! 契約種別（[`ContractType`]。月額 / ワンショット）と利用権を合わせた受講権（glossary の entitlement。
//! [`Entitlement`]）と、月額の契約（glossary の subscription。[`Subscription`]）。Gen1 で実装するのは月額だけだが、
//! ワンショットも型として持つ。月額で消費されるのはビデオ通話の暦月 2 回の枠（[`LiveLessonQuota`]）だけで、
//! 動画とチャットは送り放題。どのスポーツがどの契約種別で結べるかは [`ContractType::covers`] の 1 か所に置く。

mod contract_type;
// 1 ファイル 1 公開型の規約で、型 `Entitlement` のファイル名をコンテキスト名と同じにしている
#[allow(clippy::module_inception)]
mod entitlement;
mod entitlement_id;
mod live_lesson_quota;
mod subscription;
mod subscription_id;
mod subscription_status;
mod year_month;

pub use contract_type::ContractType;
pub use entitlement::Entitlement;
pub use entitlement_id::EntitlementId;
pub use live_lesson_quota::LiveLessonQuota;
pub use subscription::Subscription;
pub use subscription_id::SubscriptionId;
pub use subscription_status::SubscriptionStatus;
pub use year_month::YearMonth;
