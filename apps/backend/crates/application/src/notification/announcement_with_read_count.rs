use std::collections::BTreeSet;

use domain::notification::Announcement;

use super::NotificationRepository;
use crate::Unavailable;

/// 本部が送ったお知らせ 1 件と、受け取った生徒のうち既読にした数（[`super::ListAnnouncements`] と
/// [`super::WithdrawAnnouncement`] が返す）。
///
/// 既読にした数は、そのお知らせの生徒ごとのお知らせ（[`domain::notification::Notification`]）のうち既読のもの
/// （[`domain::notification::Notification::is_read`]）の数。取り下げたお知らせも同じに数える（取り下げても既読の扱いは
/// 変えない。D141）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementWithReadCount {
    pub announcement: Announcement,
    pub read_count: u32,
}

impl AnnouncementWithReadCount {
    /// 送ったばかりのお知らせ（生徒ごとのお知らせは読んでいないまま作るので、既読にした数は 0）。
    pub fn just_sent(announcement: Announcement) -> Self {
        Self {
            announcement,
            read_count: 0,
        }
    }

    /// 送った記録 `announcements` に、既読にした数を `notifications` から数えて添える（並びはそのまま）。
    pub(crate) async fn attach<N>(
        announcements: Vec<Announcement>,
        notifications: &N,
    ) -> Result<Vec<Self>, Unavailable>
    where
        N: NotificationRepository,
    {
        let ids: BTreeSet<_> = announcements.iter().map(Announcement::id).collect();
        let counts = if ids.is_empty() {
            Default::default()
        } else {
            notifications.count_read_by_announcement(&ids).await?
        };
        Ok(announcements
            .into_iter()
            .map(|announcement| {
                let read_count = counts.get(&announcement.id()).copied().unwrap_or(0);
                Self {
                    announcement,
                    read_count,
                }
            })
            .collect())
    }
}
