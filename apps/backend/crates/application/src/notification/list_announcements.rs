use domain::notification::Announcement;

use super::AnnouncementRepository;
use crate::Unavailable;

/// 本部が送ったお知らせの一覧のユースケース（admin-web の `listAnnouncements`。本部だけが呼ぶ）。照会だけで、何も保存しない。
/// 送った時刻の新しい順（同じ時刻は後に送ったものが先）で、どの本部のアカウントが送ったかで絞らない。
/// 誰が呼べるか（役割）は境界（interface）が確かめる。行動イベントは発火しない。
pub struct ListAnnouncements<A> {
    announcements: A,
}

impl<A> ListAnnouncements<A>
where
    A: AnnouncementRepository,
{
    pub fn new(announcements: A) -> Self {
        Self { announcements }
    }

    pub async fn execute(&self) -> Result<Vec<Announcement>, Unavailable> {
        self.announcements.find_all().await
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        identity::AccountId,
        notification::{AnnouncementBody, AnnouncementId, AnnouncementTitle},
    };

    use super::*;
    use crate::in_memory::InMemoryAnnouncementRepository;

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn announcement(id: u128, minute: u64) -> Announcement {
        Announcement::new(
            AnnouncementId::from_u128(id),
            AccountId::from_u128(5 + id),
            AnnouncementTitle::new(format!("題{id}")).unwrap(),
            AnnouncementBody::new("本文").unwrap(),
            (id == 2).then_some(Sport::Tennis),
            1,
            at(minute),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn lists_every_announcement_newest_first() {
        let (old, new, same_time) = (
            announcement(1, 10),
            announcement(2, 20),
            announcement(3, 20),
        );
        let repository = InMemoryAnnouncementRepository::with_announcements([
            old.clone(),
            new.clone(),
            same_time.clone(),
        ]);
        let listed = ListAnnouncements::new(repository.clone())
            .execute()
            .await
            .unwrap();
        assert_eq!(
            listed,
            [same_time, new, old],
            "新しい順。同じ時刻は後に残したものが先。送った本部のアカウントで絞らない"
        );
        assert_eq!(repository.all().len(), 3, "照会では何も変えない");
    }

    #[tokio::test]
    async fn no_announcements_is_an_empty_list() {
        let listed = ListAnnouncements::new(InMemoryAnnouncementRepository::default())
            .execute()
            .await
            .unwrap();
        assert!(listed.is_empty());
    }
}
