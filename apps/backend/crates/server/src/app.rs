//! 組み立てたアプリ（HTTP のルーターと、同じ保存先を読み書きする定期の処理）。

use std::{future::Future, pin::Pin, time::Duration};

use application::{
    clock::Clock,
    notification::{
        LessonReminderTargetLookup, NotificationRepository, NotificationSettingsRepository,
        PushSender, SendLessonReminders,
    },
};
use axum::Router;

/// レッスンのリマインドを作る定期の処理（[`SendLessonReminders`]）を回す間隔。窓は前日（24 時間前から）と 1 時間前
/// （1 時間前から）で、作るのが窓の始まりから最大でこれだけ遅れる。
pub const LESSON_REMINDER_INTERVAL: Duration = Duration::from_secs(60);

/// 組み立てたアプリ。`main` は [`App::lesson_reminders`] を tokio のタスクで回してから、[`App::router`] で待ち受ける。
/// テストは定期の処理を回さず、ルーターだけを使う（リマインドは時刻を進める試験で、ユースケースを直接呼んで確かめる）。
pub struct App {
    pub router: Router,
    /// レッスンのリマインドの定期の処理。[`LESSON_REMINDER_INTERVAL`] ごとに [`SendLessonReminders::execute`] を呼び続ける
    /// （終わらない）。1 回が失敗したら標準エラー出力に書いて、次の回にやり直す。
    pub lesson_reminders: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl App {
    pub(crate) fn new<T, S, N, P, C>(
        router: Router,
        reminders: SendLessonReminders<T, S, N, P, C>,
    ) -> Self
    where
        T: LessonReminderTargetLookup + 'static,
        S: NotificationSettingsRepository + 'static,
        N: NotificationRepository + 'static,
        P: PushSender + 'static,
        C: Clock + 'static,
    {
        Self {
            router,
            lesson_reminders: Box::pin(async move {
                let mut ticker = tokio::time::interval(LESSON_REMINDER_INTERVAL);
                loop {
                    ticker.tick().await;
                    if let Err(error) = reminders.execute().await {
                        eprintln!("lesson_reminders: {error}");
                    }
                }
            }),
        }
    }
}
