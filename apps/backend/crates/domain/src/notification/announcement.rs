use std::time::SystemTime;

use super::{
    AnnouncementBody, AnnouncementId, AnnouncementSportFilter, AnnouncementTitle,
    AnnouncementWithdrawal, NotificationError, PushNotification,
};
use crate::{catalog::Sport, identity::AccountId, scheduling::BookingPeriod};

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

    /// 送った時刻が期間 `period` に入るか（開始を含み、終了を含まない）。本部の一覧（admin-web の `listAnnouncements`）を
    /// 送った日付で絞るのに使う。期間の規則（1 日以上・上限の日数）は本部が見る予約の一覧と同じ [`BookingPeriod`] だけが
    /// 持つ。取り下げたかは問わない。
    pub fn is_sent_in(&self, period: &BookingPeriod) -> bool {
        period.includes(self.sent_at)
    }

    /// 本部の一覧（admin-web の `listAnnouncements`）の任意のスポーツの絞り込み `sport` にこのお知らせが入るか。
    /// `None` ならスポーツを問わない（すべて入る）。スポーツを付けたときは、そのスポーツの生徒に向けて送ったもの
    /// （[`Self::sport`] が同じもの）だけが入り、生徒全員に送ったもの（[`Self::sport`] が `None`）は入らない。
    /// 取り下げたかは問わない。照合は下書きと同じ [`AnnouncementSportFilter`] の 1 か所。
    pub fn is_in_sport(&self, sport: Option<Sport>) -> bool {
        AnnouncementSportFilter::new(sport).matches(self.sport)
    }

    /// 取り下げ。取り下げていなければ `None`。
    pub const fn withdrawal(&self) -> Option<&AnnouncementWithdrawal> {
        self.withdrawal.as_ref()
    }

    /// 取り下げ済みか（生徒のお知らせの一覧に出さない）。
    pub const fn is_withdrawn(&self) -> bool {
        self.withdrawal.is_some()
    }

    /// 本部の一覧（admin-web の `listAnnouncements`）の任意の取り下げの絞り込み `withdrawn` にこのお知らせが入るか。
    /// `None` なら取り下げたかを問わない（すべて入る）。`Some(true)` なら取り下げたものだけ、`Some(false)` なら
    /// 取り下げていないもの（いま生徒のお知らせの一覧に出ているもの）だけが入る。判定は [`Self::is_withdrawn`] の 1 か所。
    pub const fn is_withdrawn_as(&self, withdrawn: Option<bool>) -> bool {
        match withdrawn {
            None => true,
            Some(withdrawn) => self.is_withdrawn() == withdrawn,
        }
    }

    /// 受け取った生徒のうち既読にした数に入れる既読の時刻の上限（その時刻ちょうどまでを含む）。取り下げたお知らせは
    /// 取り下げた時刻まで（取り下げたあとに既読になったものは数えない。生徒の一覧に出ないので読まれていない）、
    /// 取り下げていなければ上限なし（`None`）。
    pub fn reads_counted_until(&self) -> Option<SystemTime> {
        self.withdrawal.map(|w| w.withdrawn_at())
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
    fn reads_are_counted_until_the_withdrawal() {
        let mut announcement = sent(None, 1).unwrap();
        assert_eq!(
            announcement.reads_counted_until(),
            None,
            "取り下げていなければ上限なし"
        );
        announcement.withdraw(HEADQUARTERS, at(20)).unwrap();
        assert_eq!(
            announcement.reads_counted_until(),
            Some(at(20)),
            "取り下げた時刻まで"
        );
    }

    #[test]
    fn a_stored_withdrawal_is_restored() {
        let withdrawal = AnnouncementWithdrawal::new(HEADQUARTERS, at(20));
        let restored = sent(None, 1).unwrap().with_withdrawal(Some(withdrawal));
        assert_eq!(restored.withdrawal(), Some(&withdrawal));
        assert!(restored.is_withdrawn());
    }

    #[test]
    fn sent_in_a_period_includes_the_start_and_excludes_the_end() {
        let announcement = sent(None, 1).unwrap();
        let period = |from: u64, to: u64| BookingPeriod::new(at(from), at(to)).unwrap();
        assert!(announcement.is_sent_in(&period(10, 11)), "開始を含む");
        assert!(announcement.is_sent_in(&period(9, 11)));
        assert!(!announcement.is_sent_in(&period(9, 10)), "終了を含まない");
        assert!(!announcement.is_sent_in(&period(11, 12)));
    }

    #[test]
    fn a_withdrawn_announcement_is_still_sent_in_its_period() {
        let mut announcement = sent(None, 1).unwrap();
        announcement.withdraw(HEADQUARTERS, at(20)).unwrap();
        assert!(
            announcement.is_sent_in(&BookingPeriod::new(at(10), at(11)).unwrap()),
            "取り下げても送った時刻で決まる"
        );
    }

    #[test]
    fn an_announcement_is_in_its_own_sport_or_any_sport() {
        let tennis = sent(Some(Sport::Tennis), 1).unwrap();
        assert!(tennis.is_in_sport(None), "絞らなければスポーツを問わない");
        assert!(tennis.is_in_sport(Some(Sport::Tennis)));
        assert!(!tennis.is_in_sport(Some(Sport::Golf)));
    }

    #[test]
    fn an_announcement_to_every_student_is_not_in_a_sport() {
        let everyone = sent(None, 1).unwrap();
        assert!(everyone.is_in_sport(None), "絞らなければ載る");
        for sport in Sport::ALL {
            assert!(
                !everyone.is_in_sport(Some(sport)),
                "生徒全員に送ったものはスポーツで絞ると入らない: {sport:?}"
            );
        }
    }

    #[test]
    fn an_announcement_not_withdrawn_is_in_the_not_withdrawn_filter_only() {
        let announcement = sent(None, 1).unwrap();
        assert!(announcement.is_withdrawn_as(None), "絞らなければ載る");
        assert!(announcement.is_withdrawn_as(Some(false)));
        assert!(!announcement.is_withdrawn_as(Some(true)));
    }

    #[test]
    fn a_withdrawn_announcement_is_in_the_withdrawn_filter_only() {
        let mut announcement = sent(None, 1).unwrap();
        announcement.withdraw(HEADQUARTERS, at(20)).unwrap();
        assert!(announcement.is_withdrawn_as(None), "絞らなければ載る");
        assert!(announcement.is_withdrawn_as(Some(true)));
        assert!(!announcement.is_withdrawn_as(Some(false)));
    }

    #[test]
    fn a_withdrawn_announcement_is_still_in_its_sport() {
        let mut announcement = sent(Some(Sport::Golf), 1).unwrap();
        announcement.withdraw(HEADQUARTERS, at(20)).unwrap();
        assert!(announcement.is_in_sport(Some(Sport::Golf)));
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
