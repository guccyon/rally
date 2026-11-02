use std::{collections::BTreeMap, future::Future};

use domain::{identity::AccountId, profile::CoachBio};

use crate::Unavailable;

/// コーチの自己紹介文の保存先のポート（profile の持ち物）。実装は infra（当面は identity の表 `identity.coaches` の
/// `bio` を読み書きする。表を分けるのは後の段。docs/adr/0009-subdomain-types.md）と、テスト用の
/// [`crate::in_memory::InMemoryAccountRepository`]。
///
/// 自己紹介文を持てるのはコーチのアカウントだけ。コーチのアカウントがなければ（ID のアカウントがないか、コーチでなければ）
/// 読み書きとも `None`。
pub trait CoachProfileRepository: Send + Sync {
    /// コーチ `coach_id` の自己紹介文。書いていなければ空（[`CoachBio::default`]）。
    fn find_coach_bio(
        &self,
        coach_id: AccountId,
    ) -> impl Future<Output = Result<Option<CoachBio>, Unavailable>> + Send;

    /// コーチ `coach_id` の自己紹介文を `bio` に置き換え、置き換えたあとの自己紹介文を返す。コーチのアカウントが
    /// なければ何も変えずに `None`。
    fn update_coach_bio(
        &self,
        coach_id: AccountId,
        bio: CoachBio,
    ) -> impl Future<Output = Result<Option<CoachBio>, Unavailable>> + Send;

    /// すべてのコーチの自己紹介文（コーチのアカウントの ID ごと）。書いていないコーチは空（[`CoachBio::default`]）で載る。
    /// コーチでないアカウントは載らない。
    fn list_coach_bios(
        &self,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, CoachBio>, Unavailable>> + Send;
}
