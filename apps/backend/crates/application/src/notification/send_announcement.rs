use domain::{
    catalog::Sport,
    identity::AccountId,
    notification::{
        Announcement, AnnouncementBody, AnnouncementTitle, Notification, NotificationError,
    },
};

use super::{
    AnnouncementRecipientLookup, AnnouncementRepository, NotificationRepository,
    NotificationSettingsRepository, PushSender,
};
use crate::{Unavailable, clock::Clock};

/// 本部からのお知らせの送信の入力。`sender_id` は認証した本部のアカウント（役割の確認は境界が行う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendAnnouncementCommand {
    pub sender_id: AccountId,
    pub title: AnnouncementTitle,
    pub body: AnnouncementBody,
    /// 対象のスポーツ。`None` なら生徒全員
    pub sport: Option<Sport>,
}

#[derive(Debug)]
pub enum SendAnnouncementError {
    /// お知らせが不変条件を満たさない（対象の生徒が 1 人もいない）
    Invalid(NotificationError),
    /// 保存先の障害
    Unavailable(Unavailable),
}

/// 本部からのお知らせの送信のユースケース（admin-web の `sendAnnouncement`。本部だけが呼ぶ）。
///
/// 対象の生徒（スポーツで絞るか、生徒全員）を [`AnnouncementRecipientLookup`] に問い合わせ、送った記録
/// （[`Announcement`]。受け取った生徒の数を持つ）を残してから、生徒ごとにお知らせ（[`Notification`]。中身は
/// [`Announcement::push_to`]）を作る。お知らせを作るたびに、その生徒の受け取り設定で本部からのお知らせがオンなら
/// プッシュ通知を送る（オフが止めるのはプッシュ通知だけで、お知らせは作る）。
///
/// - 対象の生徒がいなければ何も残さず [`SendAnnouncementError::Invalid`]
///   （[`NotificationError::AnnouncementWithoutRecipients`]）
/// - プッシュ通知は送れなくても（受け取り設定を読めない・基盤に渡せない）止めず、次の生徒に進む（お知らせは残っている）
/// - 送った記録・お知らせを残せなければ [`SendAnnouncementError::Unavailable`]。それまでに作ったお知らせは残り、
///   送り直すと別のお知らせになる
///
/// 行動イベントも境界間イベントも発火しない。
pub struct SendAnnouncement<A, L, N, S, P, C> {
    announcements: A,
    recipients: L,
    notifications: N,
    settings: S,
    sender: P,
    clock: C,
}

impl<A, L, N, S, P, C> SendAnnouncement<A, L, N, S, P, C>
where
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    pub fn new(
        announcements: A,
        recipients: L,
        notifications: N,
        settings: S,
        sender: P,
        clock: C,
    ) -> Self {
        Self {
            announcements,
            recipients,
            notifications,
            settings,
            sender,
            clock,
        }
    }

    pub async fn execute(
        &self,
        command: SendAnnouncementCommand,
    ) -> Result<Announcement, SendAnnouncementError> {
        let students = self
            .recipients
            .find_students(command.sport)
            .await
            .map_err(SendAnnouncementError::Unavailable)?;
        let now = self.clock.now();
        let announcement = Announcement::new(
            self.announcements.next_id(),
            command.sender_id,
            command.title,
            command.body,
            command.sport,
            u32::try_from(students.len()).map_err(|_| {
                SendAnnouncementError::Unavailable(Unavailable::new("生徒が多すぎる"))
            })?,
            now,
        )
        .map_err(SendAnnouncementError::Invalid)?;
        self.announcements
            .insert(announcement.clone())
            .await
            .map_err(SendAnnouncementError::Unavailable)?;
        for student_id in students {
            let push = announcement.push_to(student_id);
            self.notifications
                .insert(Notification::new(self.notifications.next_id(), push, now))
                .await
                .map_err(SendAnnouncementError::Unavailable)?;
            if let Ok(settings) = self.settings.find(student_id).await
                && settings.allows(&push)
            {
                // 送れなくてもお知らせは残っているので止めない
                let _ = self.sender.send(push).await;
            }
        }
        Ok(announcement)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Mutex,
        time::{Duration, SystemTime},
    };

    use domain::notification::{
        AnnouncementId, NotificationKind, NotificationSettings, PushNotification,
    };

    use super::*;
    use crate::in_memory::{
        FixedClock, InMemoryAnnouncementRepository, InMemoryNotificationRepository,
        InMemoryNotificationSettingsRepository, RecordingPushSender,
    };

    const HEADQUARTERS: AccountId = AccountId::from_u128(5);
    const TENNIS_STUDENT: AccountId = AccountId::from_u128(1);
    const SWIMMING_STUDENT: AccountId = AccountId::from_u128(2);
    const QUIET_STUDENT: AccountId = AccountId::from_u128(3);

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    /// テニスの生徒 1・3 と水泳の生徒 2。問い合わせたスポーツを記録する。
    #[derive(Default)]
    struct FakeRecipients {
        asked: Mutex<Vec<Option<Sport>>>,
    }

    impl AnnouncementRecipientLookup for FakeRecipients {
        async fn find_students(&self, sport: Option<Sport>) -> Result<Vec<AccountId>, Unavailable> {
            self.asked.lock().unwrap().push(sport);
            Ok([
                (TENNIS_STUDENT, Sport::Tennis),
                (SWIMMING_STUDENT, Sport::Swimming),
                (QUIET_STUDENT, Sport::Tennis),
            ]
            .into_iter()
            .filter(|(_, s)| sport.is_none_or(|sport| sport == *s))
            .map(|(id, _)| id)
            .collect())
        }
    }

    /// 送れない基盤
    struct BrokenSender;

    impl PushSender for BrokenSender {
        async fn send(&self, _notification: PushNotification) -> Result<(), Unavailable> {
            Err(Unavailable::new("送れない"))
        }
    }

    /// 残せない保存先
    struct BrokenAnnouncements;

    impl AnnouncementRepository for BrokenAnnouncements {
        fn next_id(&self) -> AnnouncementId {
            AnnouncementId::from_u128(1)
        }

        async fn insert(&self, _announcement: Announcement) -> Result<(), Unavailable> {
            Err(Unavailable::new("壊れている"))
        }

        async fn find(&self, _id: AnnouncementId) -> Result<Option<Announcement>, Unavailable> {
            unreachable!()
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

    /// 生徒 3 は本部からのお知らせのプッシュ通知をオフにしている
    fn settings() -> InMemoryNotificationSettingsRepository {
        let mut quiet = NotificationSettings::new(QUIET_STUDENT);
        quiet.set(NotificationKind::Announcement, false);
        InMemoryNotificationSettingsRepository::with_settings([quiet])
    }

    fn command(sport: Option<Sport>) -> SendAnnouncementCommand {
        SendAnnouncementCommand {
            sender_id: HEADQUARTERS,
            title: AnnouncementTitle::new("メンテナンスのお知らせ").unwrap(),
            body: AnnouncementBody::new("11/3 の 2:00〜4:00 は使えません").unwrap(),
            sport,
        }
    }

    #[allow(clippy::type_complexity)]
    fn use_case<P: PushSender>(
        sender: P,
    ) -> SendAnnouncement<
        InMemoryAnnouncementRepository,
        FakeRecipients,
        InMemoryNotificationRepository,
        InMemoryNotificationSettingsRepository,
        P,
        FixedClock,
    > {
        SendAnnouncement::new(
            InMemoryAnnouncementRepository::default(),
            FakeRecipients::default(),
            InMemoryNotificationRepository::default(),
            settings(),
            sender,
            FixedClock(now()),
        )
    }

    #[tokio::test]
    async fn sends_to_every_student_and_records_it() {
        let use_case = use_case(RecordingPushSender::default());
        let announcement = use_case.execute(command(None)).await.unwrap();

        assert_eq!(announcement.sender_id(), HEADQUARTERS);
        assert_eq!(announcement.title().as_str(), "メンテナンスのお知らせ");
        assert_eq!(announcement.sport(), None);
        assert_eq!(announcement.recipient_count(), 3);
        assert_eq!(announcement.sent_at(), now());
        assert_eq!(
            use_case.announcements.all(),
            std::slice::from_ref(&announcement)
        );
        assert_eq!(*use_case.recipients.asked.lock().unwrap(), [None]);

        let notifications = use_case.notifications.all();
        assert_eq!(
            notifications
                .iter()
                .map(Notification::push)
                .collect::<Vec<_>>(),
            [
                announcement.push_to(TENNIS_STUDENT),
                announcement.push_to(SWIMMING_STUDENT),
                announcement.push_to(QUIET_STUDENT),
            ],
            "受け取り設定でオフの生徒にもお知らせは作る"
        );
        assert!(
            notifications
                .iter()
                .all(|n| n.created_at() == now() && !n.is_read())
        );
        assert_eq!(
            use_case.sender.sent(),
            [
                announcement.push_to(TENNIS_STUDENT),
                announcement.push_to(SWIMMING_STUDENT),
            ],
            "オフの生徒にはプッシュ通知を送らない"
        );
    }

    #[tokio::test]
    async fn a_sport_narrows_the_students() {
        let use_case = use_case(RecordingPushSender::default());
        let announcement = use_case
            .execute(command(Some(Sport::Swimming)))
            .await
            .unwrap();
        assert_eq!(announcement.sport(), Some(Sport::Swimming));
        assert_eq!(announcement.recipient_count(), 1);
        assert_eq!(
            *use_case.recipients.asked.lock().unwrap(),
            [Some(Sport::Swimming)]
        );
        assert_eq!(
            use_case.sender.sent(),
            [announcement.push_to(SWIMMING_STUDENT)]
        );
    }

    #[tokio::test]
    async fn no_students_records_nothing() {
        let use_case = use_case(RecordingPushSender::default());
        let result = use_case.execute(command(Some(Sport::Golf))).await;
        assert!(matches!(
            result,
            Err(SendAnnouncementError::Invalid(
                NotificationError::AnnouncementWithoutRecipients
            ))
        ));
        assert!(use_case.announcements.all().is_empty());
        assert!(use_case.notifications.all().is_empty());
        assert!(use_case.sender.sent().is_empty());
    }

    #[tokio::test]
    async fn a_push_that_cannot_be_sent_keeps_the_notifications() {
        let use_case = use_case(BrokenSender);
        let announcement = use_case.execute(command(None)).await.unwrap();
        assert_eq!(announcement.recipient_count(), 3);
        assert_eq!(
            use_case.notifications.all().len(),
            3,
            "送れなくてもお知らせは全員に残る"
        );
    }

    #[tokio::test]
    async fn a_broken_store_is_unavailable_and_sends_nothing() {
        let use_case = SendAnnouncement::new(
            BrokenAnnouncements,
            FakeRecipients::default(),
            InMemoryNotificationRepository::default(),
            settings(),
            RecordingPushSender::default(),
            FixedClock(now()),
        );
        let result = use_case.execute(command(None)).await;
        assert!(matches!(result, Err(SendAnnouncementError::Unavailable(_))));
        assert!(use_case.notifications.all().is_empty());
        assert!(use_case.sender.sent().is_empty());
    }
}
