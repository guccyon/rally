use std::future::Future;

use domain::identity::{AccountId, Coach};

use crate::Unavailable;

/// catalog が identity にコーチ（提供スポーツ）を問い合わせるポート（メニューのスポーツはコーチの提供スポーツのどれか）。
///
/// コーチのアカウントの持ち主は identity で、catalog は読むだけ。実装は [`crate::identity::IdentityCoachLookup`]。
pub trait CoachLookup: Send + Sync {
    /// `coach_id` のコーチ。アカウントがないか、コーチでなければ `None`。
    fn find_coach(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Option<Coach>, Unavailable>> + Send;
}
