use std::collections::BTreeMap;

use domain::notification::Announcement;

use super::NotificationRepository;
use crate::Unavailable;

/// 本部が送ったお知らせ 1 件と、受け取った生徒のうち既読にした数（[`super::ListAnnouncements`] と
/// [`super::WithdrawAnnouncement`] が返す）。
///
/// 既読にした数は、そのお知らせの生徒ごとのお知らせ（[`domain::notification::Notification`]）のうち既読のもの
/// （[`domain::notification::Notification::is_read`]）の数。取り下げたお知らせは取り下げた時刻までに既読になったものだけを
/// 数える（[`Announcement::reads_counted_until`]。取り下げたあとに「すべて既読にする」で既読になったものは数えない）。
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
        let read_until: BTreeMap<_, _> = announcements
            .iter()
            .map(|announcement| (announcement.id(), announcement.reads_counted_until()))
            .collect();
        let counts = if read_until.is_empty() {
            Default::default()
        } else {
            notifications
                .count_read_by_announcement(&read_until)
                .await?
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
