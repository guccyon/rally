use std::future::Future;

use domain::{identity::AccountId, matching::FavoriteCoach};

use crate::Unavailable;

/// お気に入りのコーチの保存先のポート。実装はテスト・ローカル起動用の
/// [`crate::in_memory::InMemoryFavoriteCoachRepository`] と、infra の Postgres（`matching.favorite_coaches`）。
///
/// 同じ生徒と同じコーチの組は 1 つ（何度入れても 1 件）。
pub trait FavoriteCoachRepository: Send + Sync {
    /// お気に入りを保存する。同じ生徒とコーチの組がすでにあれば何も変えない（最初に入れた時刻のまま）。
    fn add(&self, favorite: FavoriteCoach) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// 生徒 `student_id` のお気に入りからコーチ `coach_id` を外す。入っていなければ何もしない。
    fn remove(
        &self,
        student_id: AccountId,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<(), Unavailable>> + Send;

    /// 生徒 `student_id` のお気に入りを、入れた時刻の新しい順（同じ時刻はコーチの ID の順）で返す。
    fn find_by_student(
        &self,
        student_id: AccountId,
    ) -> impl Future<Output = Result<Vec<FavoriteCoach>, Unavailable>> + Send;
}
