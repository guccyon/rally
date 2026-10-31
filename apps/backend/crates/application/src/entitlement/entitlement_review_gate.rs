use domain::{async_lesson::VideoId, catalog::Sport, entitlement::ContractType};

use crate::{Unavailable, async_lesson::ReviewGate};

/// async-lesson の [`ReviewGate`] の entitlement による実装。動画のスポーツの契約種別
/// （[`ContractType::for_sport`]）で、domain の [`ContractType::may_start_review`] に答えさせる。
///
/// 月額（ゴルフ以外）は送り放題なので常に通す。ワンショット（ゴルフ）は動画ごとの決済の確定が要るが、Gen1 は
/// ワンショットを型だけ持ち決済（billing）を実装しないので、決済が確定した動画はなく、常に通さない。
/// 決済を実装するときは、ここで動画の決済が確定しているかを billing に問い合わせる。
#[derive(Debug, Clone, Copy, Default)]
pub struct EntitlementReviewGate;

impl ReviewGate for EntitlementReviewGate {
    async fn may_start_review(
        &self,
        _video_id: VideoId,
        sport: Sport,
    ) -> Result<bool, Unavailable> {
        let payment_confirmed = false;
        Ok(ContractType::for_sport(sport).may_start_review(payment_confirmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn passes_every_sport_but_golf() {
        for sport in Sport::ALL {
            assert_eq!(
                EntitlementReviewGate
                    .may_start_review(VideoId::from_u128(1), sport)
                    .await
                    .unwrap(),
                sport != Sport::Golf,
                "{sport:?}"
            );
        }
    }
}
