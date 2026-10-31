use std::time::SystemTime;

use super::{
    AnnouncementBody, AnnouncementId, AnnouncementTitle, NotificationError, PushNotification,
};
use crate::{catalog::Sport, identity::AccountId};

/// 本部からのお知らせ（glossary の announcement）。本部が生徒全員か、あるスポーツの生徒全員に送った知らせの記録で、
/// 送った本部のアカウント・題・本文・対象のスポーツ（全員なら `None`）・受け取った生徒の数・送った時刻を持つ。
///
/// 受け取る生徒 1 人ごとにお知らせ（[`super::Notification`]。中身は [`Announcement::push_to`]）を作る。受け取る生徒は
/// 1 人以上（いなければ作らない）。予約送信・取り消しはない（送った時刻に送り終えたもの）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    id: AnnouncementId,
    sender_id: AccountId,
    title: AnnouncementTitle,
    body: AnnouncementBody,
    sport: Option<Sport>,
    recipient_count: u32,
    sent_at: SystemTime,
}

impl Announcement {
    /// 本部のアカウント `sender_id` が時刻 `sent_at` に `recipient_count` 人の生徒へ送ったお知らせ。受け取る生徒が
    /// いなければ（0 人）[`NotificationError::AnnouncementWithoutRecipients`]。保存先から読んだものを組み立てるのも同じ。
    pub fn new(
        id: AnnouncementId,
        sender_id: AccountId,
        title: AnnouncementTitle,
        body: AnnouncementBody,
        sport: Option<Sport>,
        recipient_count: u32,
        sent_at: SystemTime,
    ) -> Result<Self, NotificationError> {
        if recipient_count == 0 {
            return Err(NotificationError::AnnouncementWithoutRecipients);
        }
        Ok(Self {
            id,
            sender_id,
            title,
            body,
            sport,
            recipient_count,
            sent_at,
        })
    }

    pub fn id(&self) -> AnnouncementId {
        self.id
    }

    /// 送った本部のアカウント。
    pub fn sender_id(&self) -> AccountId {
        self.sender_id
    }

    pub fn title(&self) -> &AnnouncementTitle {
        &self.title
    }

    pub fn body(&self) -> &AnnouncementBody {
        &self.body
    }

    /// 対象のスポーツ。生徒全員に送ったなら `None`。
    pub fn sport(&self) -> Option<Sport> {
        self.sport
    }

    /// 受け取った生徒の数（1 以上）。
    pub fn recipient_count(&self) -> u32 {
        self.recipient_count
    }

    pub fn sent_at(&self) -> SystemTime {
        self.sent_at
    }

    /// 生徒 `student_id` に送るプッシュ通知（お知らせの中身）。
    pub fn push_to(&self, student_id: AccountId) -> PushNotification {
        PushNotification::Announcement {
            student_id,
            announcement_id: self.id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::notification::NotificationKind;

    const HEADQUARTERS: AccountId = AccountId::from_u128(5);
    const STUDENT: AccountId = AccountId::from_u128(1);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn sent(sport: Option<Sport>, recipient_count: u32) -> Result<Announcement, NotificationError> {
        Announcement::new(
            AnnouncementId::from_u128(7),
            HEADQUARTERS,
            AnnouncementTitle::new("メンテナンスのお知らせ").unwrap(),
            AnnouncementBody::new("11/3 の 2:00〜4:00 は使えません").unwrap(),
            sport,
            recipient_count,
            at(10),
        )
    }

    #[test]
    fn keeps_what_headquarters_sent() {
        let announcement = sent(Some(Sport::Tennis), 3).unwrap();
        assert_eq!(announcement.id(), AnnouncementId::from_u128(7));
        assert_eq!(announcement.sender_id(), HEADQUARTERS);
        assert_eq!(announcement.title().as_str(), "メンテナンスのお知らせ");
        assert_eq!(
            announcement.body().as_str(),
            "11/3 の 2:00〜4:00 は使えません"
        );
        assert_eq!(announcement.sport(), Some(Sport::Tennis));
        assert_eq!(announcement.recipient_count(), 3);
        assert_eq!(announcement.sent_at(), at(10));
        assert_eq!(sent(None, 1).unwrap().sport(), None, "全員なら None");
    }

    #[test]
    fn an_announcement_needs_at_least_one_recipient() {
        assert_eq!(
            sent(None, 0),
            Err(NotificationError::AnnouncementWithoutRecipients)
        );
    }

    #[test]
    fn the_push_goes_to_the_student_as_an_announcement() {
        let push = sent(None, 1).unwrap().push_to(STUDENT);
        assert_eq!(
            push,
            PushNotification::Announcement {
                student_id: STUDENT,
                announcement_id: AnnouncementId::from_u128(7),
            }
        );
        assert_eq!(push.recipient(), STUDENT);
        assert_eq!(push.kind(), NotificationKind::Announcement);
    }
}
