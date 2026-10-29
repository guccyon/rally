use std::{collections::HashSet, error::Error, sync::Mutex};

use domain::notification::PushNotification;
use uuid::Uuid;

use super::{NotificationSettingsRepository, PushSender};
use crate::domain_event::{DomainEvent, DomainEventSubscriber};

/// notification による境界間イベントの購読。予約の確定（`BookingConfirmed`）で生徒とコーチに 1 件ずつ、添削の返却
/// （`VideoReviewReturned`）で生徒に、メッセージの送信（`MessageSent`）で受け手（送り手でない側の当事者）にプッシュ通知を
/// 送る（[`PushNotification`]。受け取る人は [`PushNotification::recipient`]）。発行元のコンテキストは notification を直接
/// 呼ばない（context-map の向き）。
///
/// 送る前に受け取る人の受け取り設定（[`NotificationSettingsRepository`]）を読み、その通知の種類
/// （[`PushNotification::kind`]）がオフなら送らない。
///
/// 通知は保存しないので、同じイベントの再送で 2 回送らないように、送った通知を境界間イベントの `event_id` と組にして
/// プロセスのメモリに覚える。受け取り設定がオフで送らなかった通知も同じく覚え、再送では（その間に設定をオンに
/// 戻していても）送らない。1 つのイベントで複数の通知を送るときは通知ごとに覚え、送れなかった通知（受け取り設定を
/// 読めなかったときを含む）だけを再送で送り直す。ほかのイベントは何もしない。
pub struct PushNotificationSubscriber<P, S> {
    sender: P,
    settings: S,
    sent: Mutex<HashSet<(Uuid, PushNotification)>>,
}

impl<P, S> PushNotificationSubscriber<P, S>
where
    P: PushSender,
    S: NotificationSettingsRepository,
{
    pub fn new(sender: P, settings: S) -> Self {
        Self {
            sender,
            settings,
            sent: Mutex::default(),
        }
    }

    pub fn sender(&self) -> &P {
        &self.sender
    }

    /// `event_id` のイベントの通知 `notifications` を、それぞれ一度だけ扱う。同じ `event_id` の同じ通知を扱っている
    /// 途中・扱った後ならその通知は何もしない。受け取る人の受け取り設定でその種類がオフなら送らずに扱い済みにする。
    /// 1 件が送れなくても残りは送り、送れなかったときは最初の障害を返す。
    async fn send_once(
        &self,
        event_id: Uuid,
        notifications: &[PushNotification],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut first_error: Option<Box<dyn Error + Send + Sync>> = None;
        for &notification in notifications {
            let key = (event_id, notification);
            if !self.sent.lock().unwrap().insert(key) {
                continue;
            }
            let result = match self.settings.find(notification.recipient()).await {
                Ok(settings) if !settings.allows(&notification) => Ok(()),
                Ok(_) => self.sender.send(notification).await,
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                self.sent.lock().unwrap().remove(&key);
                first_error.get_or_insert(error.into());
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl<P, S> DomainEventSubscriber for PushNotificationSubscriber<P, S>
where
    P: PushSender,
    S: NotificationSettingsRepository,
{
    async fn handle(&self, event: &DomainEvent) -> Result<(), Box<dyn Error + Send + Sync>> {
        match *event {
            DomainEvent::BookingConfirmed {
                event_id,
                student_id,
                coach_id,
                booking_id,
                ..
            } => {
                self.send_once(
                    event_id,
                    &[
                        PushNotification::BookingConfirmed {
                            student_id,
                            coach_id,
                            booking_id,
                        },
                        PushNotification::BookingReceived {
                            student_id,
                            coach_id,
                            booking_id,
                        },
                    ],
                )
                .await
            }
            DomainEvent::VideoReviewReturned {
                event_id,
                student_id,
                coach_id,
                video_id,
                review_id,
                ..
            } => {
                self.send_once(
                    event_id,
                    &[PushNotification::VideoReviewReturned {
                        student_id,
                        coach_id,
                        video_id,
                        review_id,
                    }],
                )
                .await
            }
            DomainEvent::MessageSent {
                event_id,
                student_id,
                coach_id,
                thread_id,
                message_id,
                sender_role,
                ..
            } => {
                self.send_once(
                    event_id,
                    &[PushNotification::MessageReceived {
                        student_id,
                        coach_id,
                        thread_id,
                        message_id,
                        sender: sender_role,
                    }],
                )
                .await
            }
            DomainEvent::BookingCancelled { .. }
            | DomainEvent::LessonCompleted { .. }
            | DomainEvent::SubscriptionStarted { .. }
            | DomainEvent::VideoReviewRequested { .. } => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        async_lesson::{VideoId, VideoReviewId},
        identity::AccountId,
        messaging::{MessageId, MessageSender, MessageThreadId},
        notification::{NotificationKind, NotificationSettings},
        scheduling::BookingId,
    };

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{InMemoryNotificationSettingsRepository, RecordingPushSender},
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(9);

    /// 誰も受け取り設定を変えていない（すべてオン）保存先
    fn all_on() -> InMemoryNotificationSettingsRepository {
        InMemoryNotificationSettingsRepository::default()
    }

    fn oct26() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_972_800)
    }

    fn confirmed(event_id: Uuid, booking: u128) -> DomainEvent {
        DomainEvent::BookingConfirmed {
            event_id,
            occurred_at: oct26(),
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(booking),
        }
    }

    /// 予約の確定を生徒に知らせる通知
    fn to_student(booking: u128) -> PushNotification {
        PushNotification::BookingConfirmed {
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(booking),
        }
    }

    /// 予約の確定をコーチに新しい予約として知らせる通知
    fn to_coach(booking: u128) -> PushNotification {
        PushNotification::BookingReceived {
            student_id: STUDENT,
            coach_id: COACH,
            booking_id: BookingId::from_u128(booking),
        }
    }

    #[tokio::test]
    async fn a_confirmed_booking_notifies_the_student_and_the_coach_once_per_event() {
        let subscriber = PushNotificationSubscriber::new(RecordingPushSender::default(), all_on());
        let first = Uuid::new_v4();
        subscriber.handle(&confirmed(first, 1)).await.unwrap();
        subscriber.handle(&confirmed(first, 1)).await.unwrap();
        subscriber
            .handle(&confirmed(Uuid::new_v4(), 2))
            .await
            .unwrap();

        let sent = subscriber.sender().sent();
        assert_eq!(
            sent,
            [to_student(1), to_coach(1), to_student(2), to_coach(2)],
            "1 イベントで生徒とコーチに 1 件ずつ。同じイベントの再送では送らない"
        );
        assert_eq!(
            sent[..2]
                .iter()
                .map(PushNotification::recipient)
                .collect::<Vec<_>>(),
            [STUDENT, COACH]
        );
    }

    #[tokio::test]
    async fn a_returned_review_notifies_the_student_once_per_event() {
        let subscriber = PushNotificationSubscriber::new(RecordingPushSender::default(), all_on());
        let event = DomainEvent::VideoReviewReturned {
            event_id: Uuid::new_v4(),
            occurred_at: oct26(),
            student_id: STUDENT,
            coach_id: COACH,
            video_id: VideoId::from_u128(3),
            review_id: VideoReviewId::from_u128(4),
        };
        subscriber.handle(&event).await.unwrap();
        subscriber.handle(&event).await.unwrap();

        let sent = subscriber.sender().sent();
        assert_eq!(
            sent,
            [PushNotification::VideoReviewReturned {
                student_id: STUDENT,
                coach_id: COACH,
                video_id: VideoId::from_u128(3),
                review_id: VideoReviewId::from_u128(4),
            }],
            "同じイベントの再送では送らない"
        );
        assert_eq!(sent[0].recipient(), STUDENT);
    }

    #[tokio::test]
    async fn a_sent_message_notifies_the_party_who_did_not_send_it() {
        let subscriber = PushNotificationSubscriber::new(RecordingPushSender::default(), all_on());
        let sent = |message: u128, sender_role| DomainEvent::MessageSent {
            event_id: Uuid::from_u128(message),
            occurred_at: oct26(),
            student_id: STUDENT,
            coach_id: COACH,
            thread_id: MessageThreadId::from_u128(3),
            message_id: MessageId::from_u128(message),
            sender_role,
        };
        subscriber
            .handle(&sent(1, MessageSender::Student))
            .await
            .unwrap();
        subscriber
            .handle(&sent(1, MessageSender::Student))
            .await
            .unwrap();
        subscriber
            .handle(&sent(2, MessageSender::Coach))
            .await
            .unwrap();

        let notifications = subscriber.sender().sent();
        assert_eq!(
            notifications,
            [
                PushNotification::MessageReceived {
                    student_id: STUDENT,
                    coach_id: COACH,
                    thread_id: MessageThreadId::from_u128(3),
                    message_id: MessageId::from_u128(1),
                    sender: MessageSender::Student,
                },
                PushNotification::MessageReceived {
                    student_id: STUDENT,
                    coach_id: COACH,
                    thread_id: MessageThreadId::from_u128(3),
                    message_id: MessageId::from_u128(2),
                    sender: MessageSender::Coach,
                },
            ],
            "同じイベントの再送では送らない"
        );
        assert_eq!(
            notifications
                .iter()
                .map(PushNotification::recipient)
                .collect::<Vec<_>>(),
            [COACH, STUDENT],
            "生徒が送ればコーチに、コーチが送れば生徒に届く"
        );
    }

    #[tokio::test]
    async fn other_events_send_nothing() {
        let subscriber = PushNotificationSubscriber::new(RecordingPushSender::default(), all_on());
        subscriber
            .handle(&DomainEvent::BookingCancelled {
                event_id: Uuid::new_v4(),
                occurred_at: oct26(),
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: BookingId::from_u128(1),
                consumes_quota: false,
                cancelled_at: oct26(),
            })
            .await
            .unwrap();
        assert!(subscriber.sender().sent().is_empty());
    }

    /// `down_for` にあるアカウントへの通知には障害を返し、それ以外は送った通知を記録する送り先。
    #[derive(Default)]
    struct Flaky {
        down_for: Mutex<HashSet<AccountId>>,
        sent: RecordingPushSender,
    }

    impl Flaky {
        fn set_down_for(&self, accounts: &[AccountId]) {
            *self.down_for.lock().unwrap() = accounts.iter().copied().collect();
        }
    }

    impl PushSender for Flaky {
        async fn send(&self, notification: PushNotification) -> Result<(), Unavailable> {
            if self
                .down_for
                .lock()
                .unwrap()
                .contains(&notification.recipient())
            {
                return Err(Unavailable::new("push gateway is down"));
            }
            self.sent.send(notification).await
        }
    }

    #[tokio::test]
    async fn a_failed_send_is_retried_when_the_event_comes_again() {
        let subscriber = PushNotificationSubscriber::new(Flaky::default(), all_on());
        subscriber.sender().set_down_for(&[STUDENT, COACH]);
        let event = confirmed(Uuid::new_v4(), 1);
        assert!(subscriber.handle(&event).await.is_err());
        assert!(subscriber.sender().sent.sent().is_empty());

        subscriber.sender().set_down_for(&[]);
        subscriber.handle(&event).await.unwrap();
        assert_eq!(
            subscriber.sender().sent.sent(),
            [to_student(1), to_coach(1)]
        );
    }

    #[tokio::test]
    async fn only_the_notification_that_failed_is_sent_again() {
        let subscriber = PushNotificationSubscriber::new(Flaky::default(), all_on());
        let event = confirmed(Uuid::new_v4(), 1);

        // 生徒への通知だけ送れない。コーチへの通知は送る
        subscriber.sender().set_down_for(&[STUDENT]);
        assert!(subscriber.handle(&event).await.is_err());
        assert_eq!(subscriber.sender().sent.sent(), [to_coach(1)]);

        // 再送では生徒への通知だけを送り直す
        subscriber.sender().set_down_for(&[]);
        subscriber.handle(&event).await.unwrap();
        subscriber.handle(&event).await.unwrap();
        assert_eq!(
            subscriber.sender().sent.sent(),
            [to_coach(1), to_student(1)],
            "送れたコーチへの通知は送り直さない"
        );

        // コーチへの通知だけ送れなかったときも同じ
        let other = confirmed(Uuid::new_v4(), 2);
        subscriber.sender().set_down_for(&[COACH]);
        assert!(subscriber.handle(&other).await.is_err());
        subscriber.sender().set_down_for(&[]);
        subscriber.handle(&other).await.unwrap();
        assert_eq!(
            subscriber.sender().sent.sent()[2..],
            [to_student(2), to_coach(2)]
        );
    }

    /// 保存先 `settings` で、アカウント `account_id` の種類 `kind` を受け取るか（`enabled`）を変える
    async fn turned(
        settings: &InMemoryNotificationSettingsRepository,
        account_id: AccountId,
        kind: NotificationKind,
        enabled: bool,
    ) {
        settings
            .update(account_id, |s| s.set(kind, enabled))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_kind_turned_off_is_not_sent_even_when_the_event_comes_again() {
        let settings = all_on();
        let subscriber =
            PushNotificationSubscriber::new(RecordingPushSender::default(), settings.clone());
        turned(
            &settings,
            STUDENT,
            NotificationKind::BookingConfirmed,
            false,
        )
        .await;

        let event = confirmed(Uuid::new_v4(), 1);
        subscriber.handle(&event).await.unwrap();
        assert_eq!(
            subscriber.sender().sent(),
            [to_coach(1)],
            "オフにした生徒には送らず、コーチ（設定はオンのまま）には送る"
        );

        // 送らなかったことは覚えているので、設定をオンに戻しても同じイベントの再送では送らない
        turned(&settings, STUDENT, NotificationKind::BookingConfirmed, true).await;
        subscriber.handle(&event).await.unwrap();
        assert_eq!(subscriber.sender().sent(), [to_coach(1)]);

        // オンに戻したあとの新しいイベントは送る
        subscriber
            .handle(&confirmed(Uuid::new_v4(), 2))
            .await
            .unwrap();
        assert_eq!(
            subscriber.sender().sent(),
            [to_coach(1), to_student(2), to_coach(2)]
        );
    }

    #[tokio::test]
    async fn only_the_kind_turned_off_is_suppressed() {
        let settings = all_on();
        let subscriber =
            PushNotificationSubscriber::new(RecordingPushSender::default(), settings.clone());
        turned(&settings, STUDENT, NotificationKind::MessageReceived, false).await;

        let returned = PushNotification::VideoReviewReturned {
            student_id: STUDENT,
            coach_id: COACH,
            video_id: VideoId::from_u128(3),
            review_id: VideoReviewId::from_u128(4),
        };
        subscriber
            .handle(&DomainEvent::VideoReviewReturned {
                event_id: Uuid::new_v4(),
                occurred_at: oct26(),
                student_id: STUDENT,
                coach_id: COACH,
                video_id: VideoId::from_u128(3),
                review_id: VideoReviewId::from_u128(4),
            })
            .await
            .unwrap();
        subscriber
            .handle(&DomainEvent::MessageSent {
                event_id: Uuid::new_v4(),
                occurred_at: oct26(),
                student_id: STUDENT,
                coach_id: COACH,
                thread_id: MessageThreadId::from_u128(3),
                message_id: MessageId::from_u128(1),
                sender_role: MessageSender::Coach,
            })
            .await
            .unwrap();
        assert_eq!(
            subscriber.sender().sent(),
            [returned],
            "メッセージだけオフ。添削の返却は送る"
        );
    }

    /// `down` のあいだ読み取りに障害を返し、それ以外は中の保存先を読む受け取り設定の保存先。
    #[derive(Default)]
    struct FlakySettings {
        down: Mutex<bool>,
        settings: InMemoryNotificationSettingsRepository,
    }

    impl NotificationSettingsRepository for FlakySettings {
        async fn find(&self, account_id: AccountId) -> Result<NotificationSettings, Unavailable> {
            if *self.down.lock().unwrap() {
                return Err(Unavailable::new("settings store is down"));
            }
            self.settings.find(account_id).await
        }

        async fn update<F>(
            &self,
            account_id: AccountId,
            update: F,
        ) -> Result<NotificationSettings, Unavailable>
        where
            F: FnOnce(&mut NotificationSettings) + Send,
        {
            self.settings.update(account_id, update).await
        }
    }

    #[tokio::test]
    async fn a_notification_whose_settings_could_not_be_read_is_sent_on_redelivery() {
        let subscriber = PushNotificationSubscriber::new(
            RecordingPushSender::default(),
            FlakySettings::default(),
        );
        let event = confirmed(Uuid::new_v4(), 1);

        *subscriber.settings.down.lock().unwrap() = true;
        assert!(subscriber.handle(&event).await.is_err());
        assert!(
            subscriber.sender().sent().is_empty(),
            "設定を読めなければ送らない"
        );

        *subscriber.settings.down.lock().unwrap() = false;
        subscriber.handle(&event).await.unwrap();
        assert_eq!(
            subscriber.sender().sent(),
            [to_student(1), to_coach(1)],
            "再送で送り直す"
        );
    }
}
