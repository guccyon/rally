use std::collections::BTreeSet;

use domain::notification::{Announcement, Notification};

use super::AnnouncementRepository;
use crate::Unavailable;

/// 自分のお知らせ 1 件（[`super::ListMyNotifications`] と [`super::MarkMyNotificationsRead`] が返す）。本部からのお知らせ
/// （[`domain::notification::PushNotification::Announcement`]）なら、その送った記録を添える（表示する題と本文は境界が
/// ここから決める）。ほかの種類は `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyNotification {
    pub notification: Notification,
    pub announcement: Option<Announcement>,
}

impl MyNotification {
    /// お知らせ `notifications` に、本部からのお知らせの送った記録を `announcements` から読んで添える（並びはそのまま）。
    /// 送った記録が見つからないお知らせがあれば、保存先が壊れているとして障害にする。
    pub(crate) async fn attach<A>(
        notifications: Vec<Notification>,
        announcements: &A,
    ) -> Result<Vec<Self>, Unavailable>
    where
        A: AnnouncementRepository,
    {
        let ids: BTreeSet<_> = notifications
            .iter()
            .filter_map(|notification| notification.push().announcement_id())
            .collect();
        let found = if ids.is_empty() {
            Default::default()
        } else {
            announcements.find_by_ids(&ids).await?
        };
        notifications
            .into_iter()
            .map(|notification| {
                let announcement = match notification.push().announcement_id() {
                    Some(id) => Some(found.get(&id).cloned().ok_or_else(|| {
                        Unavailable::new(format!(
                            "お知らせ {:?} の本部からのお知らせ {id:?} がない",
                            notification.id()
                        ))
                    })?),
                    None => None,
                };
                Ok(Self {
                    notification,
                    announcement,
                })
            })
            .collect()
    }
}
