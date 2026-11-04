use std::time::SystemTime;

use super::{NotificationError, NotificationId, NotificationKind, PushNotification};
use crate::identity::AccountId;

/// お知らせ（glossary の notification）。送ったプッシュ通知（[`PushNotification`]）を、受け取る人があとで一覧で
/// 見られるように残したもの。中身のプッシュ通知と、作った時刻と、受け取る人が読んだ時刻（既読。読むまでは `None`）を持つ。
///
/// 受け取り設定（[`super::NotificationSettings`]）でその種類をオフにしていても作る（オフが止めるのはプッシュ通知だけ）。
/// 表示する本文は境界のアダプタがプッシュ通知の中身から決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notification {
    id: NotificationId,
    push: PushNotification,
    created_at: SystemTime,
    read_at: Option<SystemTime>,
}

impl Notification {
    /// 時刻 `created_at` に作った、まだ読んでいないお知らせ。
    pub fn new(id: NotificationId, push: PushNotification, created_at: SystemTime) -> Self {
        Self {
            id,
            push,
            created_at,
            read_at: None,
        }
    }

    /// 保存先から読んだお知らせを組み立てる（保存先の実装用）。既読が作った時刻より前なら
    /// [`NotificationError::NotificationReadBeforeCreated`]（作った時刻と同じなら組み立てる）。
    pub fn restore(
        id: NotificationId,
        push: PushNotification,
        created_at: SystemTime,
        read_at: Option<SystemTime>,
    ) -> Result<Self, NotificationError> {
        if read_at.is_some_and(|read_at| read_at < created_at) {
            return Err(NotificationError::NotificationReadBeforeCreated);
        }
        Ok(Self {
            id,
            push,
            created_at,
            read_at,
        })
    }

    pub fn id(&self) -> NotificationId {
        self.id
    }

    /// 残したプッシュ通知。
    pub fn push(&self) -> PushNotification {
        self.push
    }

    /// お知らせを受け取る人（プッシュ通知を受け取る人と同じ）。
    pub fn recipient(&self) -> AccountId {
        self.push.recipient()
    }

    /// お知らせの種類（プッシュ通知の種類と同じ）。
    pub fn kind(&self) -> NotificationKind {
        self.push.kind()
    }

    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    pub fn read_at(&self) -> Option<SystemTime> {
        self.read_at
    }

    pub fn is_read(&self) -> bool {
        self.read_at.is_some()
    }

    /// 本部が見る生徒 1 人の受け取ったお知らせ（admin-web の `listStudentAnnouncements`）の任意の絞り込み `unread_only` に
    /// このお知らせが入るか。`false` なら既読かを問わない（すべて入る）。`true` なら未読のもの（[`Self::is_read`] でないもの）
    /// だけが入る。判定は [`Self::is_read`] の 1 か所（Postgres の実装は同じ条件を `read_at IS NULL` で絞る）。
    pub fn is_unread_as(&self, unread_only: bool) -> bool {
        !unread_only || !self.is_read()
    }

    /// 時刻 `at` に読んだことにする。すでに読んでいれば最初に読んだ時刻のまま。`at` より後に作ったお知らせ
    /// （既読にする操作と並んで作られたもの）は読んでいないままにする。
    pub fn mark_read(&mut self, at: SystemTime) {
        if self.read_at.is_none() && self.created_at <= at {
            self.read_at = Some(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::scheduling::BookingId;

    const STUDENT: AccountId = AccountId::from_u128(1);
    const COACH: AccountId = AccountId::from_u128(9);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    fn confirmed() -> Notification {
        Notification::new(
            NotificationId::from_u128(5),
            PushNotification::BookingConfirmed {
                student_id: STUDENT,
                coach_id: COACH,
                booking_id: BookingId::from_u128(3),
            },
            at(10),
        )
    }

    #[test]
    fn a_new_notification_is_unread_and_follows_its_push() {
        let notification = confirmed();
        assert_eq!(notification.id(), NotificationId::from_u128(5));
        assert_eq!(notification.recipient(), STUDENT);
        assert_eq!(notification.kind(), NotificationKind::BookingConfirmed);
        assert_eq!(notification.created_at(), at(10));
        assert_eq!(notification.read_at(), None);
        assert!(!notification.is_read());
    }

    #[test]
    fn marking_read_keeps_the_first_time() {
        let mut notification = confirmed();
        notification.mark_read(at(20));
        assert_eq!(notification.read_at(), Some(at(20)));
        assert!(notification.is_read());
        notification.mark_read(at(30));
        assert_eq!(
            notification.read_at(),
            Some(at(20)),
            "最初に読んだ時刻のまま"
        );
    }

    #[test]
    fn the_unread_only_filter_takes_only_unread_notifications() {
        let mut notification = confirmed();
        assert!(notification.is_unread_as(false), "絞らなければ載る");
        assert!(notification.is_unread_as(true), "未読は載る");
        notification.mark_read(at(20));
        assert!(notification.is_unread_as(false), "絞らなければ既読も載る");
        assert!(!notification.is_unread_as(true), "既読は載らない");
    }

    #[test]
    fn a_notification_created_after_the_read_stays_unread() {
        let mut notification = confirmed();
        notification.mark_read(at(9));
        assert_eq!(notification.read_at(), None);
        notification.mark_read(at(10));
        assert_eq!(
            notification.read_at(),
            Some(at(10)),
            "作った時刻と同じなら読める"
        );
    }

    #[test]
    fn restore_rejects_a_read_before_creation_and_accepts_the_same_time() {
        let push = confirmed().push();
        let id = NotificationId::from_u128(5);
        assert_eq!(
            Notification::restore(id, push, at(10), Some(at(9))),
            Err(NotificationError::NotificationReadBeforeCreated)
        );
        let same = Notification::restore(id, push, at(10), Some(at(10))).unwrap();
        assert_eq!(
            same.read_at(),
            Some(at(10)),
            "作った時刻と同じなら組み立てる"
        );
        let unread = Notification::restore(id, push, at(10), None).unwrap();
        assert_eq!(unread, confirmed(), "既読でなければ new と同じ");
    }
}
