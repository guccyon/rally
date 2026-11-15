use std::time::Duration;

use domain::notification::{LessonReminderTiming, Notification, PushNotification};

use super::{
    LessonReminderTargetLookup, NotificationRepository, NotificationSettingsRepository, PushSender,
};
use crate::{Unavailable, clock::Clock};

/// レッスンのリマインドを作るユースケース（定期の処理。組み立て用のクレートが決まった間隔で呼ぶ）。境界間イベントでなく
/// 時刻の経過がきっかけで、今の時刻は [`Clock`] だけから読む（時刻を進める試験と、触れる環境の `CLOCK_OFFSET_SECONDS`
/// で確かめられるように）。
///
/// 1 回の処理で、今の時刻にリマインドの窓が開いている（[`LessonReminderTiming::due_at`]）確定した同期レッスンの予約を
/// scheduling に問い合わせ（[`LessonReminderTargetLookup`]。キャンセル・コーチのお休みで消えた予約と確定待ちは入らない）、
/// 予約ごとに生徒とコーチへのリマインド（[`PushNotification::lesson_reminders`]）をお知らせとして残す。生徒の受け取り設定で
/// リマインドがオフなら、その生徒にはお知らせも作らない（[`domain::notification::NotificationSettings::records`]）。
/// コーチには常に作る。新しく残したお知らせだけ、受け取り設定に従ってプッシュ通知を送る（送れなくてもお知らせは残る）。
///
/// 同じ窓で何度回っても重複しない: お知らせは (予約, 時点, 宛先) で 1 件で、守るのは保存先
/// （[`NotificationRepository::insert`]。すでにあれば残さず `false`）。処理の側に送った記録は持たない。処理が止まっていて
/// 窓を過ぎたリマインドは作らない。1 件が残せなくても残りは続け、最初の障害を返す（次の回にやり直す）。
pub struct SendLessonReminders<T, S, N, P, C> {
    targets: T,
    settings: S,
    notifications: N,
    sender: P,
    clock: C,
}

impl<T, S, N, P, C> SendLessonReminders<T, S, N, P, C>
where
    T: LessonReminderTargetLookup,
    S: NotificationSettingsRepository,
    N: NotificationRepository,
    P: PushSender,
    C: Clock,
{
    pub fn new(targets: T, settings: S, notifications: N, sender: P, clock: C) -> Self {
        Self {
            targets,
            settings,
            notifications,
            sender,
            clock,
        }
    }

    /// 今の時刻に窓が開いているリマインドを作り、新しく残したお知らせの数を返す。
    pub async fn execute(&self) -> Result<usize, Unavailable> {
        let now = self.clock.now();
        // 開始が今から前日の窓の長さまでの予約（境目の判定は domain。問い合わせは少し広めに取る）
        let until = now + LessonReminderTiming::DayBefore.lead() + Duration::from_secs(1);
        let targets = self
            .targets
            .confirmed_bookings_starting_between(now, until)
            .await?;
        let mut created = 0;
        let mut first_error = None;
        for target in targets {
            let Some(timing) = LessonReminderTiming::due_at(target.starts_at, now) else {
                continue;
            };
            for push in PushNotification::lesson_reminders(
                target.student_id,
                target.coach_id,
                target.booking_id,
                timing,
            ) {
                match self.remind(push).await {
                    Ok(true) => created += 1,
                    Ok(false) => {}
                    Err(error) => {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
        first_error.map_or(Ok(created), Err)
    }

    /// リマインド `push` を、受け取る人の受け取り設定が許せばお知らせとして残し、新しく残したら送る。新しく残したかを返す。
    async fn remind(&self, push: PushNotification) -> Result<bool, Unavailable> {
        let settings = self.settings.find(push.recipient()).await?;
        if !settings.records(&push) {
            return Ok(false);
        }
        let inserted = self
            .notifications
            .insert(Notification::new(
                self.notifications.next_id(),
                push,
                self.clock.now(),
            ))
            .await?;
        if inserted && settings.allows(&push) {
            // 送れなくてもお知らせは残っているので止めない（作り直さないので、送り直しもしない）
            let _ = self.sender.send(push).await;
        }
        Ok(inserted)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::SystemTime,
    };

    use domain::{
        identity::AccountId,
        notification::{NotificationKind, NotificationSettings},
        scheduling::BookingId,
    };

    use super::*;
    use crate::{
        in_memory::{
            InMemoryNotificationRepository, InMemoryNotificationSettingsRepository, ManualClock,
            RecordingPushSender,
        },
        notification::LessonReminderTarget,
    };

    const STUDENT: AccountId = AccountId::from_u128(1);
    const OTHER_STUDENT: AccountId = AccountId::from_u128(2);
    const COACH: AccountId = AccountId::from_u128(9);
    const BOOKING: BookingId = BookingId::from_u128(30);
    const OTHER_BOOKING: BookingId = BookingId::from_u128(31);
    const HOUR: Duration = Duration::from_secs(60 * 60);

    fn at(hour: u32) -> SystemTime {
        SystemTime::UNIX_EPOCH + HOUR * hour
    }

    /// 確定した予約の一覧を差し替えられる対象（キャンセルで外れるのを表す）。
    #[derive(Clone, Default)]
    struct Targets(Arc<Mutex<Vec<LessonReminderTarget>>>);

    impl Targets {
        fn set(&self, targets: impl IntoIterator<Item = LessonReminderTarget>) {
            *self.0.lock().unwrap() = targets.into_iter().collect();
        }
    }

    impl LessonReminderTargetLookup for Targets {
        async fn confirmed_bookings_starting_between(
            &self,
            from: SystemTime,
            until: SystemTime,
        ) -> Result<Vec<LessonReminderTarget>, Unavailable> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|t| from <= t.starts_at && t.starts_at < until)
                .copied()
                .collect())
        }
    }

    fn target(booking_id: BookingId, student_id: AccountId, hour: u32) -> LessonReminderTarget {
        LessonReminderTarget {
            booking_id,
            student_id,
            coach_id: COACH,
            starts_at: at(hour),
        }
    }

    struct Fixture {
        targets: Targets,
        settings: InMemoryNotificationSettingsRepository,
        notifications: InMemoryNotificationRepository,
        sender: RecordingPushSender,
        clock: ManualClock,
        use_case: SendLessonReminders<
            Targets,
            InMemoryNotificationSettingsRepository,
            InMemoryNotificationRepository,
            RecordingPushSender,
            ManualClock,
        >,
    }

    fn fixture(settings: impl IntoIterator<Item = NotificationSettings>) -> Fixture {
        let targets = Targets::default();
        let settings = InMemoryNotificationSettingsRepository::with_settings(settings);
        let notifications = InMemoryNotificationRepository::default();
        let sender = RecordingPushSender::default();
        let clock = ManualClock::new(at(0));
        Fixture {
            use_case: SendLessonReminders::new(
                targets.clone(),
                settings.clone(),
                notifications.clone(),
                sender.clone(),
                clock.clone(),
            ),
            targets,
            settings,
            notifications,
            sender,
            clock,
        }
    }

    impl Fixture {
        /// 残したお知らせのプッシュ通知（残した順）。
        fn pushes(&self) -> Vec<PushNotification> {
            self.notifications.all().iter().map(|n| n.push()).collect()
        }
    }

    fn reminders(booking_id: BookingId, timing: LessonReminderTiming) -> [PushNotification; 2] {
        PushNotification::lesson_reminders(STUDENT, COACH, booking_id, timing)
    }

    #[tokio::test]
    async fn advancing_the_clock_creates_one_reminder_per_window_and_recipient() {
        let f = fixture([]);
        f.targets.set([target(BOOKING, STUDENT, 100)]);

        f.clock.set(at(100) - HOUR * 24 - Duration::from_secs(1));
        assert_eq!(f.use_case.execute().await.unwrap(), 0, "前日の窓の前");

        f.clock.set(at(100) - HOUR * 24);
        assert_eq!(f.use_case.execute().await.unwrap(), 2);
        let day_before = reminders(BOOKING, LessonReminderTiming::DayBefore);
        assert_eq!(f.pushes(), day_before);
        assert_eq!(
            f.notifications.all()[0].created_at(),
            at(100) - HOUR * 24,
            "作った時刻は時計の時刻"
        );

        f.clock.set(at(100) - HOUR);
        assert_eq!(f.use_case.execute().await.unwrap(), 2);
        let hour_before = reminders(BOOKING, LessonReminderTiming::HourBefore);
        assert_eq!(f.pushes(), [day_before, hour_before].concat());
        assert_eq!(
            f.sender.sent(),
            [day_before, hour_before].concat(),
            "新しく残したものを送る"
        );

        f.clock.set(at(100));
        assert_eq!(f.use_case.execute().await.unwrap(), 0, "始まったら作らない");
        assert_eq!(f.notifications.all().len(), 4);
    }

    #[tokio::test]
    async fn running_again_in_the_same_window_creates_nothing_more() {
        let f = fixture([]);
        f.targets.set([target(BOOKING, STUDENT, 100)]);
        f.clock.set(at(90));
        assert_eq!(f.use_case.execute().await.unwrap(), 2);
        for minute in [0, 1, 30, 59] {
            f.clock.set(at(90) + Duration::from_secs(minute * 60));
            assert_eq!(f.use_case.execute().await.unwrap(), 0);
        }
        assert_eq!(
            f.pushes(),
            reminders(BOOKING, LessonReminderTiming::DayBefore)
        );
        assert_eq!(f.sender.sent().len(), 2, "送り直さない");
    }

    #[tokio::test]
    async fn a_booking_that_is_no_longer_confirmed_gets_no_more_reminders() {
        let f = fixture([]);
        f.targets.set([target(BOOKING, STUDENT, 100)]);
        f.clock.set(at(80));
        f.use_case.execute().await.unwrap();

        // キャンセル・コーチのお休みで、問い合わせの結果から外れる
        f.targets.set([]);
        f.clock.set(at(99));
        assert_eq!(f.use_case.execute().await.unwrap(), 0);
        assert_eq!(
            f.pushes(),
            reminders(BOOKING, LessonReminderTiming::DayBefore),
            "1 時間前は作らない"
        );
    }

    #[tokio::test]
    async fn a_student_who_turned_reminders_off_gets_none_but_the_coach_still_does() {
        let mut off = NotificationSettings::new(STUDENT);
        off.set(NotificationKind::LessonReminder, false);
        let f = fixture([off]);
        f.targets.set([
            target(BOOKING, STUDENT, 100),
            target(OTHER_BOOKING, OTHER_STUDENT, 100),
        ]);
        f.clock.set(at(99));
        assert_eq!(f.use_case.execute().await.unwrap(), 3);

        let [_, to_coach] = reminders(BOOKING, LessonReminderTiming::HourBefore);
        let others = PushNotification::lesson_reminders(
            OTHER_STUDENT,
            COACH,
            OTHER_BOOKING,
            LessonReminderTiming::HourBefore,
        );
        let mut pushes = f.pushes();
        pushes.sort_by_key(|p| (p.lesson_reminder(), p.recipient()));
        assert_eq!(pushes, [to_coach, others[0], others[1]]);

        // あとでオンに戻しても、作らなかったリマインドは作り直さない（窓を過ぎたもの）
        f.settings
            .update(STUDENT, |settings| {
                settings.set(NotificationKind::LessonReminder, true)
            })
            .await
            .unwrap();
        f.clock.set(at(100));
        assert_eq!(f.use_case.execute().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_booking_confirmed_inside_the_hour_gets_only_the_hour_before_reminder() {
        let f = fixture([]);
        f.clock.set(at(100) - HOUR / 2);
        f.targets.set([target(BOOKING, STUDENT, 100)]);
        f.use_case.execute().await.unwrap();
        assert_eq!(
            f.pushes(),
            reminders(BOOKING, LessonReminderTiming::HourBefore),
            "過ぎた前日の窓のリマインドは作らない"
        );
    }
}
