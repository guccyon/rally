use std::future::Future;

use domain::identity::AccountId;

use crate::Unavailable;

/// profile が matching に、生徒がコーチをお気に入りに入れているかを問い合わせるポート（コーチの紹介に添える）。
///
/// お気に入りの持ち主は matching で、profile は読むだけ。実装は [`crate::matching::MatchingCoachFavoriteLookup`]。
pub trait CoachFavoriteLookup: Send + Sync {
    /// 生徒 `student_id` のお気に入りにコーチ `coach_id` が入っているか。
    fn is_favorite_coach(
        &self,
        student_id: AccountId,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
