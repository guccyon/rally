use std::future::Future;

use domain::{async_lesson::VideoId, identity::Sport};

use crate::Unavailable;

/// async-lesson が「この動画の添削に着手してよいか」を問い合わせるポート（不変条件 2: ゴルフの添削は
/// ワンショット決済が確定した動画にしか着手できない）。
///
/// 判定の規則は domain の [`domain::entitlement::ContractType::may_start_review`] にあり、async-lesson は答えに
/// 従うだけで契約種別も決済も知らない。実装は [`crate::entitlement::EntitlementReviewGate`]。添削の返却
/// （[`super::ReturnVideoReview`]）が返却の前に呼ぶ（Gen1 は着手と返却を分けないので、返却が着手を兼ねる）。
pub trait ReviewGate: Send + Sync {
    /// `video_id` の動画（スポーツは `sport`）の添削に着手してよいか。
    fn may_start_review(
        &self,
        video_id: VideoId,
        sport: Sport,
    ) -> impl Future<Output = Result<bool, Unavailable>> + Send;
}
