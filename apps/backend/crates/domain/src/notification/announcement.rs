use std::time::SystemTime;

use super::{
    AnnouncementBody, AnnouncementId, AnnouncementTitle, AnnouncementWithdrawal, NotificationError,
    PushNotification,
};
use crate::{catalog::Sport, identity::AccountId};

/// 本部からのお知らせ（glossary の announcement）。本部が生徒全員か、あるスポーツの生徒全員に送った知らせの記録で、
/// 送った本部のアカウント・題・本文・対象のスポーツ（全員なら `None`）・受け取った生徒の数・送った時刻を持つ。
///
/// 受け取る生徒 1 人ごとにお知らせ（[`super::Notification`]。中身は [`Announcement::push_to`]）を作る。受け取る生徒は
/// 1 人以上（いなければ作らない）。予約送信はない（送った時刻に送り終えたもの）。誤って送ったものは本部が取り下げられ
/// （[`Announcement::withdraw`]）、取り下げたものは生徒のお知らせの一覧に出さない。送った中身は取り下げても変えない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    id: AnnouncementId,
    sender_id: AccountId,
    title: AnnouncementTitle,
    body: AnnouncementBody,
    sport: Option<Sport>,
    recipient_count: u32,
    sent_at: SystemTime,
    withdrawal: Option<AnnouncementWithdrawal>,
}

impl Announcement {
    /// 本部のアカウント `sender_id` が時刻 `sent_at` に `recipient_count` 人の生徒へ送ったお知らせ。受け取る生徒が
    /// いなければ（0 人）[`NotificationError::AnnouncementWithoutRecipients`]。保存先から読んだものを組み立てるのも同じ
    /// （取り下げは [`Self::with_withdrawal`] で添える。既定はなし）。
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
            withdrawal: None,
        })
    }

    /// 取り下げ `withdrawal` を添える（保存先の実装用）。
    pub const fn with_withdrawal(mut self, withdrawal: Option<AnnouncementWithdrawal>) -> Self {
        self.withdrawal = withdrawal;
        self
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

    /// 取り下げ。取り下げていなければ `None`。
    pub const fn withdrawal(&self) -> Option<&AnnouncementWithdrawal> {
        self.withdrawal.as_ref()
    }

    /// 取り下げ済みか（生徒のお知らせの一覧に出さない）。
    pub const fn is_withdrawn(&self) -> bool {
        self.withdrawal.is_some()
    }

    /// 本部のアカウント `withdrawn_by` が時刻 `now` にお知らせを取り下げる。送った中身と受け取った生徒の数は変えない。
    ///
    /// すでに取り下げてあれば [`NotificationError::AnnouncementAlreadyWithdrawn`] で、何も変えない（最初の取り下げを残す）。
    pub fn withdraw(
        &mut self,
        withdrawn_by: AccountId,
        now: SystemTime,
    ) -> Result<(), NotificationError> {
        if self.withdrawal.is_some() {
            return Err(NotificationError::AnnouncementAlreadyWithdrawn);
        }
        self.withdrawal = Some(AnnouncementWithdrawal::new(withdrawn_by, now));
        Ok(())
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
    const OTHER_HEADQUARTERS: AccountId = AccountId::from_u128(6);
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
        assert_eq!(
            announcement.withdrawal(),
            None,
            "送ったときは取り下げていない"
        );
        assert!(!announcement.is_withdrawn());
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
    fn headquarters_withdraws_an_announcement_keeping_what_was_sent() {
        let mut announcement = sent(Some(Sport::Tennis), 3).unwrap();
        let before = announcement.clone();
        announcement.withdraw(OTHER_HEADQUARTERS, at(20)).unwrap();
        assert!(announcement.is_withdrawn());
        assert_eq!(
            announcement.withdrawal(),
            Some(&AnnouncementWithdrawal::new(OTHER_HEADQUARTERS, at(20)))
        );
        assert_eq!(
            announcement.clone().with_withdrawal(None),
            before,
            "送った中身・受け取った生徒の数は変えない"
        );
    }

    #[test]
    fn withdrawing_twice_is_rejected_and_keeps_the_first_withdrawal() {
        let mut announcement = sent(None, 1).unwrap();
        announcement.withdraw(HEADQUARTERS, at(20)).unwrap();
        let first = announcement.clone();
        assert_eq!(
            announcement.withdraw(OTHER_HEADQUARTERS, at(30)),
            Err(NotificationError::AnnouncementAlreadyWithdrawn)
        );
        assert_eq!(announcement, first, "何も変えない");
    }

    #[test]
    fn a_stored_withdrawal_is_restored() {
        let withdrawal = AnnouncementWithdrawal::new(HEADQUARTERS, at(20));
        let restored = sent(None, 1).unwrap().with_withdrawal(Some(withdrawal));
        assert_eq!(restored.withdrawal(), Some(&withdrawal));
        assert!(restored.is_withdrawn());
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
