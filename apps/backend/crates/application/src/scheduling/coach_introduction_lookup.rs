use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use domain::identity::AccountId;

use super::CoachIntroduction;
use crate::Unavailable;

/// scheduling が profile にコーチの紹介を問い合わせるポート（スロットの検索で枠にコーチの表示名と自己紹介文を添える）。
///
/// 表示名・自己紹介文の持ち主は profile で、scheduling は読むだけ。実装は
/// [`crate::profile::ProfileCoachIntroductionLookup`]。
pub trait CoachIntroductionLookup: Send + Sync {
    /// `coach_ids` のコーチの紹介。コーチのアカウントがない ID は結果に含めない。
    fn find_coach_introductions(
        &self,
        coach_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, CoachIntroduction>, Unavailable>> + Send;
}
