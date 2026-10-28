use std::collections::BTreeSet;

use super::{NotificationKind, PushNotification};
use crate::identity::AccountId;

/// 通知の受け取り設定。アカウントごとに、通知の種類（[`NotificationKind`]）ごとに受け取るか（オン / オフ）を持つ。
/// 既定はすべてオンで、オフにした種類だけを覚える。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationSettings {
    account_id: AccountId,
    disabled: BTreeSet<NotificationKind>,
}

impl NotificationSettings {
    /// 既定の受け取り設定（すべてオン）。
    pub fn new(account_id: AccountId) -> Self {
        Self::restore(account_id, [])
    }

    /// 保存した受け取り設定を戻す。`disabled` はオフにした種類で、それ以外はオン。
    pub fn restore(
        account_id: AccountId,
        disabled: impl IntoIterator<Item = NotificationKind>,
    ) -> Self {
        Self {
            account_id,
            disabled: disabled.into_iter().collect(),
        }
    }

    pub fn account_id(&self) -> AccountId {
        self.account_id
    }

    /// 種類 `kind` の通知を受け取るか。
    pub fn is_enabled(&self, kind: NotificationKind) -> bool {
        !self.disabled.contains(&kind)
    }

    /// 種類 `kind` の通知を受け取るか（`enabled`）を決める。
    pub fn set(&mut self, kind: NotificationKind, enabled: bool) {
        if enabled {
            self.disabled.remove(&kind);
        } else {
            self.disabled.insert(kind);
        }
    }

    /// 通知 `notification` を送ってよいか（その種類がオンか）。受け取る人がこの設定のアカウントかは確かめない。
    pub fn allows(&self, notification: &PushNotification) -> bool {
        self.is_enabled(notification.kind())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduling::BookingId;

    const STUDENT: AccountId = AccountId::from_u128(1);

    #[test]
    fn every_kind_is_enabled_by_default() {
        let settings = NotificationSettings::new(STUDENT);
        assert_eq!(settings.account_id(), STUDENT);
        for kind in NotificationKind::ALL {
            assert!(settings.is_enabled(kind), "{kind:?} は既定でオン");
        }
        assert_eq!(settings, NotificationSettings::restore(STUDENT, []));
    }

    #[test]
    fn a_kind_can_be_turned_off_and_on_again() {
        let mut settings = NotificationSettings::new(STUDENT);
        settings.set(NotificationKind::MessageReceived, false);
        assert!(!settings.is_enabled(NotificationKind::MessageReceived));
        assert!(settings.is_enabled(NotificationKind::BookingConfirmed));
        assert!(settings.is_enabled(NotificationKind::VideoReviewReturned));
        assert_eq!(
            settings,
            NotificationSettings::restore(STUDENT, [NotificationKind::MessageReceived])
        );

        settings.set(NotificationKind::MessageReceived, true);
        assert_eq!(settings, NotificationSettings::new(STUDENT));
    }

    #[test]
    fn allows_only_notifications_of_enabled_kinds() {
        let confirmed = PushNotification::BookingConfirmed {
            student_id: STUDENT,
            coach_id: AccountId::from_u128(9),
            booking_id: BookingId::from_u128(3),
        };
        let mut settings = NotificationSettings::new(STUDENT);
        assert!(settings.allows(&confirmed));
        settings.set(NotificationKind::BookingConfirmed, false);
        assert!(!settings.allows(&confirmed));
    }
}
