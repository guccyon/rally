use std::future::Future;

use domain::catalog::Sport;

use super::CoachCandidate;
use crate::Unavailable;

/// matching が profile に、あるスポーツを提供するコーチを問い合わせるポート。
///
/// 提供スポーツ・表示名の持ち主は profile で、matching は読むだけ。実装は
/// [`crate::profile::ProfileCoachCandidateLookup`]。
pub trait CoachCandidateLookup: Send + Sync {
    /// `sport` を提供するコーチをすべて（並べない。並べるのはユースケース）。
    fn find_coaches_offering(
        &self,
        sport: Sport,
    ) -> impl Future<Output = Result<Vec<CoachCandidate>, Unavailable>> + Send;
}
