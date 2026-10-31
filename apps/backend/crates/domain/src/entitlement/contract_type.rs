use super::EntitlementError;
use crate::catalog::Sport;

/// 契約種別（specs/product-context.md の課金の 2 種類）。
///
/// 外部での文字列表現（契約の `ContractType`・DB の `contract_type`）は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContractType {
    /// 月額（glossary の subscription）。ゴルフ以外。動画とチャットは送り放題、ビデオ通話は暦月の上限（契約プランで決まる）まで
    Monthly,
    /// ワンショット（glossary の one_shot）。ゴルフだけ。動画添削 1 回ごとの都度課金
    OneShot,
}

impl ContractType {
    /// この契約種別で `sport` の契約を結べるか。月額はゴルフ以外、ワンショットはゴルフだけ。
    pub const fn covers(self, sport: Sport) -> bool {
        match self {
            ContractType::Monthly => !matches!(sport, Sport::Golf),
            ContractType::OneShot => matches!(sport, Sport::Golf),
        }
    }

    /// [`Self::covers`] でなければ [`EntitlementError::SportNotCoveredByContract`]。
    pub(crate) const fn ensure_covers(self, sport: Sport) -> Result<(), EntitlementError> {
        if self.covers(sport) {
            Ok(())
        } else {
            Err(EntitlementError::SportNotCoveredByContract {
                contract_type: self,
                sport,
            })
        }
    }

    /// 動画とチャットが送り放題か（月額だけ）。
    pub const fn unlimited_video_and_chat(self) -> bool {
        matches!(self, ContractType::Monthly)
    }

    /// `sport` の動画添削を受ける契約種別（[`Self::covers`] の逆。ゴルフはワンショット、それ以外は月額）。
    pub const fn for_sport(sport: Sport) -> Self {
        if ContractType::OneShot.covers(sport) {
            ContractType::OneShot
        } else {
            ContractType::Monthly
        }
    }

    /// この契約種別の動画の添削に着手してよいか（不変条件 2）。`payment_confirmed` はその動画の決済が
    /// 確定しているか。
    ///
    /// 月額は送り放題なので常に `true`。ワンショット（ゴルフ）は動画 1 回ごとの都度課金なので、決済が
    /// 確定した動画だけ。
    pub const fn may_start_review(self, payment_confirmed: bool) -> bool {
        match self {
            ContractType::Monthly => true,
            ContractType::OneShot => payment_confirmed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_covers_every_sport_but_golf() {
        for sport in Sport::ALL {
            assert_eq!(
                ContractType::Monthly.covers(sport),
                sport != Sport::Golf,
                "{sport:?}"
            );
        }
    }

    #[test]
    fn one_shot_covers_only_golf() {
        for sport in Sport::ALL {
            assert_eq!(
                ContractType::OneShot.covers(sport),
                sport == Sport::Golf,
                "{sport:?}"
            );
        }
    }

    #[test]
    fn golf_videos_are_reviewed_under_one_shot_and_the_rest_under_monthly() {
        for sport in Sport::ALL {
            let contract_type = ContractType::for_sport(sport);
            assert!(contract_type.covers(sport), "{sport:?}");
            assert_eq!(
                contract_type == ContractType::OneShot,
                sport == Sport::Golf,
                "{sport:?}"
            );
        }
    }

    #[test]
    fn swimming_is_monthly_and_never_one_shot() {
        // 変更シナリオ cs-001 で足した水泳は、規則を変えずに月額（ゴルフ以外）に入る
        assert!(ContractType::Monthly.covers(Sport::Swimming));
        assert!(!ContractType::OneShot.covers(Sport::Swimming));
        assert_eq!(
            ContractType::for_sport(Sport::Swimming),
            ContractType::Monthly
        );
    }

    #[test]
    fn one_shot_reviews_start_only_after_the_payment_is_confirmed() {
        assert!(ContractType::Monthly.may_start_review(false));
        assert!(ContractType::Monthly.may_start_review(true));
        assert!(!ContractType::OneShot.may_start_review(false));
        assert!(ContractType::OneShot.may_start_review(true));
    }

    #[test]
    fn only_monthly_has_unlimited_video_and_chat() {
        assert!(ContractType::Monthly.unlimited_video_and_chat());
        assert!(!ContractType::OneShot.unlimited_video_and_chat());
    }
}
