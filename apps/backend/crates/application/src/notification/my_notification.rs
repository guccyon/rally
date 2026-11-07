use std::collections::BTreeSet;

use domain::notification::{Announcement, Notification, PushNotification};

use super::{AnnouncementRepository, CoachLeaveNotice, CoachLeaveNoticeLookup};
use crate::Unavailable;

/// 自分のお知らせ 1 件（[`super::ListMyNotifications`] と [`super::MarkMyNotificationsRead`] が返す）。本部からのお知らせ
/// （[`domain::notification::PushNotification::Announcement`]）なら、その送った記録を添える（表示する題と本文は境界が
/// ここから決める）。ほかの種類は `None`。本部による予約のキャンセル
/// （[`domain::notification::PushNotification::BookingCancelledByHeadquarters`]）なら、お休みの記録から日時と一言を添える
/// （引けなければ `None` で、境界は日時のない本文にする）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyNotification {
    pub notification: Notification,
    pub announcement: Option<Announcement>,
    pub coach_leave: Option<CoachLeaveNotice>,
}

impl MyNotification {
    /// お知らせ `notifications` に、本部からのお知らせの送った記録を `announcements` から、本部による予約のキャンセルの
    /// 日時と一言を `coach_leaves` から読んで添える（並びはそのまま）。本部が取り下げた（[`Announcement::is_withdrawn`]）
    /// お知らせは外す（お知らせそのものと既読は保存先に残る）。送った記録が見つからないお知らせがあれば、保存先が壊れている
    /// として障害にする。
    pub(crate) async fn attach<A, L>(
        notifications: Vec<Notification>,
        announcements: &A,
        coach_leaves: &L,
    ) -> Result<Vec<Self>, Unavailable>
    where
        A: AnnouncementRepository,
        L: CoachLeaveNoticeLookup,
    {
        let cancelled: BTreeSet<_> = notifications
            .iter()
            .filter_map(|notification| match notification.push() {
                PushNotification::BookingCancelledByHeadquarters { booking_id, .. } => {
                    Some(booking_id)
                }
                _ => None,
            })
            .collect();
        let mut notices = if cancelled.is_empty() {
            Default::default()
        } else {
            coach_leaves.find_coach_leave_notices(&cancelled).await?
        };
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
                let coach_leave = match notification.push() {
                    PushNotification::BookingCancelledByHeadquarters { booking_id, .. } => {
                        notices.remove(&booking_id)
                    }
                    _ => None,
                };
                Ok(Self {
                    notification,
                    announcement,
                    coach_leave,
                })
            })
            .filter(|mine: &Result<Self, Unavailable>| {
                !matches!(mine, Ok(Self { announcement: Some(a), .. }) if a.is_withdrawn())
            })
            .collect()
    }
}
