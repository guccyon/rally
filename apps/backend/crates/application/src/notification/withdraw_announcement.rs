use domain::{
    identity::AccountId,
    notification::{Announcement, AnnouncementId},
};

use super::{AnnouncementRepository, WithdrawAnnouncementError};
use crate::clock::Clock;

/// 本部によるお知らせの取り下げのユースケース（admin-web の `withdrawAnnouncement`。本部だけが呼ぶ）。誰が呼べるか
/// （本部だけ）は境界（interface）が確かめる。
///
/// 送った記録を読み、domain の [`Announcement::withdraw`] で取り下げて（取り下げた本部のアカウントと今の時刻を残す）
/// 保存し、取り下げたあとの記録を返す。ここでは判定しない。拒否では何も保存しない。
/// 生徒ごとのお知らせは消さず、生徒のお知らせの一覧（[`super::ListMyNotifications`]）が取り下げたものを外す。
/// 既読は変えず、プッシュ通知も送らない。行動イベントも境界間イベントも発火しない（本部の運営の操作）。
pub struct WithdrawAnnouncement<A, C> {
    announcements: A,
    clock: C,
}

impl<A, C> WithdrawAnnouncement<A, C>
where
    A: AnnouncementRepository,
    C: Clock,
{
    pub fn new(announcements: A, clock: C) -> Self {
        Self {
            announcements,
            clock,
        }
    }

    /// `withdrawn_by` は認証した本部のアカウント。
    pub async fn execute(
        &self,
        announcement_id: AnnouncementId,
        withdrawn_by: AccountId,
    ) -> Result<Announcement, WithdrawAnnouncementError> {
        let mut announcement = self
            .announcements
            .find(announcement_id)
            .await?
            .ok_or(WithdrawAnnouncementError::AnnouncementNotFound)?;
        announcement
            .withdraw(withdrawn_by, self.clock.now())
            .map_err(WithdrawAnnouncementError::Invalid)?;
        self.announcements.update(announcement.clone()).await?;
        Ok(announcement)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::notification::{
        AnnouncementBody, AnnouncementTitle, AnnouncementWithdrawal, NotificationError,
    };

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{FixedClock, InMemoryAnnouncementRepository},
    };

    const HEADQUARTERS: AccountId = AccountId::from_u128(5);
    const OTHER_HEADQUARTERS: AccountId = AccountId::from_u128(6);
    const ANNOUNCEMENT: AnnouncementId = AnnouncementId::from_u128(7);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn sent() -> Announcement {
        Announcement::new(
            ANNOUNCEMENT,
            HEADQUARTERS,
            AnnouncementTitle::new("メンテナンスのお知らせ").unwrap(),
            AnnouncementBody::new("11/3 は使えません").unwrap(),
            None,
            2,
            at(10),
        )
        .unwrap()
    }

    fn use_case(
        repository: InMemoryAnnouncementRepository,
        minute: u64,
    ) -> WithdrawAnnouncement<InMemoryAnnouncementRepository, FixedClock> {
        WithdrawAnnouncement::new(repository, FixedClock(at(minute)))
    }

    #[tokio::test]
    async fn headquarters_withdraws_an_announcement_now() {
        let repository = InMemoryAnnouncementRepository::with_announcements([sent()]);
        let withdrawn = use_case(repository.clone(), 20)
            .execute(ANNOUNCEMENT, OTHER_HEADQUARTERS)
            .await
            .unwrap();
        assert_eq!(
            withdrawn.withdrawal(),
            Some(&AnnouncementWithdrawal::new(OTHER_HEADQUARTERS, at(20))),
            "送った本部のアカウントでなくても取り下げられる。取り下げたのは呼んだ本部のアカウント"
        );
        assert_eq!(withdrawn.clone().with_withdrawal(None), sent());
        assert_eq!(repository.all(), [withdrawn], "取り下げを残す");
    }

    #[tokio::test]
    async fn withdrawing_twice_is_rejected_and_keeps_the_first_withdrawal() {
        let repository = InMemoryAnnouncementRepository::with_announcements([sent()]);
        let first = use_case(repository.clone(), 20)
            .execute(ANNOUNCEMENT, HEADQUARTERS)
            .await
            .unwrap();
        let result = use_case(repository.clone(), 30)
            .execute(ANNOUNCEMENT, OTHER_HEADQUARTERS)
            .await;
        assert!(matches!(
            result,
            Err(WithdrawAnnouncementError::Invalid(
                NotificationError::AnnouncementAlreadyWithdrawn
            ))
        ));
        assert_eq!(repository.all(), [first], "最初の取り下げのまま");
    }

    #[tokio::test]
    async fn a_missing_announcement_is_not_found() {
        let repository = InMemoryAnnouncementRepository::with_announcements([sent()]);
        let result = use_case(repository.clone(), 20)
            .execute(AnnouncementId::from_u128(8), HEADQUARTERS)
            .await;
        assert!(matches!(
            result,
            Err(WithdrawAnnouncementError::AnnouncementNotFound)
        ));
        assert_eq!(repository.all(), [sent()], "何も変えない");
    }

    /// 読めない保存先
    struct BrokenAnnouncements;

    impl AnnouncementRepository for BrokenAnnouncements {
        fn next_id(&self) -> AnnouncementId {
            ANNOUNCEMENT
        }

        async fn insert(&self, _announcement: Announcement) -> Result<(), Unavailable> {
            unreachable!()
        }

        async fn find(&self, _id: AnnouncementId) -> Result<Option<Announcement>, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }

        async fn update(&self, _announcement: Announcement) -> Result<(), Unavailable> {
            unreachable!()
        }

        async fn find_all(&self) -> Result<Vec<Announcement>, Unavailable> {
            unreachable!()
        }

        async fn find_by_ids(
            &self,
            _ids: &std::collections::BTreeSet<AnnouncementId>,
        ) -> Result<std::collections::BTreeMap<AnnouncementId, Announcement>, Unavailable> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn a_broken_store_is_unavailable() {
        let result = WithdrawAnnouncement::new(BrokenAnnouncements, FixedClock(at(20)))
            .execute(ANNOUNCEMENT, HEADQUARTERS)
            .await;
        assert!(matches!(
            result,
            Err(WithdrawAnnouncementError::Unavailable(_))
        ));
    }
}
