//! matching: マッチング（specs/context-map.yaml。検索・推薦・リクエスト・担当コーチの確定）。
//!
//! 生徒が気に入ったコーチを残しておくお気に入り（[`FavoriteCoach`]。glossary の favorite_coach）。お気に入りに入れられるのは
//! 生徒だけで、入れる相手はコーチだけ。同じ生徒と同じコーチの組は 1 つ（何度入れても 1 件）で、保存先がその組で守る。

mod favorite_coach;
mod matching_error;

pub use favorite_coach::FavoriteCoach;
pub use matching_error::MatchingError;
