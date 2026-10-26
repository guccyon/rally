use crate::{DomainError, identity::Sport};

/// 契約種別（specs/product-context.md の課金の 2 種類）。
///
/// 外部での文字列表現（契約の `ContractType`・DB の `contract_type`）は境界のアダプタが決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContractType {
    /// 月額（glossary の subscription）。ゴルフ以外。動画とチャットは送り放題、ビデオ通話は暦月 2 回まで
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

    /// [`Self::covers`] でなければ [`DomainError::SportNotCoveredByContract`]。
    pub(crate) const fn ensure_covers(self, sport: Sport) -> Result<(), DomainError> {
        if self.covers(sport) {
            Ok(())
        } else {
            Err(DomainError::SportNotCoveredByContract {
                contract_type: self,
                sport,
            })
        }
    }

    /// 動画とチャットが送り放題か（月額だけ）。
    pub const fn unlimited_video_and_chat(self) -> bool {
        matches!(self, ContractType::Monthly)
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
    fn only_monthly_has_unlimited_video_and_chat() {
        assert!(ContractType::Monthly.unlimited_video_and_chat());
        assert!(!ContractType::OneShot.unlimited_video_and_chat());
    }
}
