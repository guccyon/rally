use domain::{
    identity::AccountId,
    notification::{Announcement, AnnouncementDraftId},
};

use super::{
    AnnouncementDraftError, AnnouncementDraftRepository, AnnouncementRecipientLookup,
    AnnouncementRepository, NotificationRepository, NotificationSettingsRepository, PushSender,
    SendAnnouncement, SendAnnouncementCommand, SendAnnouncementError,
};
use crate::clock::Clock;

/// 本部からのお知らせの下書きから送るユースケース（admin-web の `sendAnnouncementDraft`。本部だけが呼ぶ）。誰が呼べるか
/// （本部だけ）は境界（interface）が確かめる。どの本部のアカウントが作った下書きでも送れ、送ったのは呼んだ本部のアカウント。
///
/// 下書きを読み、題・本文を送るお知らせと同じ規則で確かめ（[`domain::notification::AnnouncementDraft::sendable`]）、
/// 送るのは [`SendAnnouncement`] に任せる（対象の生徒・お知らせ・プッシュ通知の扱いは同じ）。送れたら下書きを消し、
/// 送った記録を返す。送った記録は本部の一覧（[`super::ListAnnouncements`]）に出る。
///
/// - ない下書きは [`AnnouncementDraftError::DraftNotFound`]
/// - 題・本文が空なら [`AnnouncementDraftError::Invalid`]（[`domain::notification::NotificationError::EmptyAnnouncementTitle`]
///   など）、対象の生徒がいなければ [`AnnouncementDraftError::Invalid`]
///   （[`domain::notification::NotificationError::AnnouncementWithoutRecipients`]）。どちらも何も送らず、下書きは残す
/// - 送れたあとに下書きを消せなくても送ったことは失敗にしない（下書きは残り、本部が消せる）
///
/// 行動イベントも境界間イベントも発火しない。
pub struct SendAnnouncementDraft<D, A, L, N, S, P, C> {
    drafts: D,
    send_announcement: SendAnnouncement<A, L, N, S, P, C>,
}

impl<D, A, L, N, S, P, C> SendAnnouncementDraft<D, A, L, N, S, P, C>
where
    D: AnnouncementDraftRepository,
    A: AnnouncementRepository,
    L: AnnouncementRecipientLookup,
    N: NotificationRepository,
    S: NotificationSettingsRepository,
    P: PushSender,
    C: Clock,
{
    pub fn new(drafts: D, send_announcement: SendAnnouncement<A, L, N, S, P, C>) -> Self {
        Self {
            drafts,
            send_announcement,
        }
    }

    /// `sender_id` は認証した本部のアカウント。
    pub async fn execute(
        &self,
        draft_id: AnnouncementDraftId,
        sender_id: AccountId,
    ) -> Result<Announcement, AnnouncementDraftError> {
        let draft = self
            .drafts
            .find(draft_id)
            .await?
            .ok_or(AnnouncementDraftError::DraftNotFound)?;
        let (title, body) = draft.sendable().map_err(AnnouncementDraftError::Invalid)?;
        let announcement = self
            .send_announcement
            .execute(SendAnnouncementCommand {
                sender_id,
                title,
                body,
                sport: draft.sport(),
            })
            .await
            .map_err(|error| match error {
                SendAnnouncementError::Invalid(error) => AnnouncementDraftError::Invalid(error),
                SendAnnouncementError::Unavailable(error) => {
                    AnnouncementDraftError::Unavailable(error)
                }
            })?;
        // 送ったお知らせは残っているので、下書きを消せなくても止めない
        let _ = self.drafts.delete(draft_id).await;
        Ok(announcement)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use domain::{
        catalog::Sport,
        notification::{AnnouncementDraft, Notification, NotificationError},
    };

    use super::*;
    use crate::{
        Unavailable,
        in_memory::{
            FixedClock, InMemoryAnnouncementDraftRepository, InMemoryAnnouncementRepository,
            InMemoryNotificationRepository, InMemoryNotificationSettingsRepository,
            RecordingPushSender,
        },
    };

    const AUTHOR: AccountId = AccountId::from_u128(5);
    const SENDER: AccountId = AccountId::from_u128(6);
    const TENNIS_STUDENT: AccountId = AccountId::from_u128(1);
    const SWIMMING_STUDENT: AccountId = AccountId::from_u128(2);
    const DRAFT: AnnouncementDraftId = AnnouncementDraftId::from_u128(7);

    fn at(minute: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(minute * 60)
    }

    /// テニスの生徒 1 と水泳の生徒 2。
    struct FakeRecipients;

    impl AnnouncementRecipientLookup for FakeRecipients {
        async fn find_students(&self, sport: Option<Sport>) -> Result<Vec<AccountId>, Unavailable> {
            Ok([
                (TENNIS_STUDENT, Sport::Tennis),
                (SWIMMING_STUDENT, Sport::Swimming),
            ]
            .into_iter()
            .filter(|(_, s)| sport.is_none_or(|sport| sport == *s))
            .map(|(id, _)| id)
            .collect())
        }
    }

    /// 消せない下書きの保存先（読むのは `draft`）
    struct UndeletableDrafts(AnnouncementDraft);

    impl AnnouncementDraftRepository for UndeletableDrafts {
        fn next_id(&self) -> AnnouncementDraftId {
            unreachable!()
        }

        async fn insert(&self, _draft: AnnouncementDraft) -> Result<(), Unavailable> {
            unreachable!()
        }

        async fn find(
            &self,
            _id: AnnouncementDraftId,
        ) -> Result<Option<AnnouncementDraft>, Unavailable> {
            Ok(Some(self.0.clone()))
        }

        async fn update(&self, _draft: AnnouncementDraft) -> Result<bool, Unavailable> {
            unreachable!()
        }

        async fn delete(&self, _id: AnnouncementDraftId) -> Result<bool, Unavailable> {
            Err(Unavailable::new("壊れている"))
        }

        async fn find_all(&self) -> Result<Vec<AnnouncementDraft>, Unavailable> {
            unreachable!()
        }
    }

    fn draft(title: &str, body: &str, sport: Option<Sport>) -> AnnouncementDraft {
        AnnouncementDraft::new(DRAFT, AUTHOR, title, body, sport, at(10)).unwrap()
    }

    /// テストが中身を読む保存先と送り先。
    #[derive(Default)]
    struct Stores {
        announcements: InMemoryAnnouncementRepository,
        notifications: InMemoryNotificationRepository,
        push: RecordingPushSender,
    }

    #[allow(clippy::type_complexity)]
    fn use_case<D: AnnouncementDraftRepository>(
        drafts: D,
        stores: &Stores,
    ) -> SendAnnouncementDraft<
        D,
        InMemoryAnnouncementRepository,
        FakeRecipients,
        InMemoryNotificationRepository,
        InMemoryNotificationSettingsRepository,
        RecordingPushSender,
        FixedClock,
    > {
        SendAnnouncementDraft::new(
            drafts,
            SendAnnouncement::new(
                stores.announcements.clone(),
                FakeRecipients,
                stores.notifications.clone(),
                InMemoryNotificationSettingsRepository::default(),
                stores.push.clone(),
                FixedClock(at(30)),
            ),
        )
    }

    #[tokio::test]
    async fn sends_the_draft_and_deletes_it() {
        let stores = Stores::default();
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([draft(
            "メンテナンスのお知らせ",
            "11/3 は使えません",
            Some(Sport::Swimming),
        )]);
        let announcement = use_case(drafts.clone(), &stores)
            .execute(DRAFT, SENDER)
            .await
            .unwrap();

        assert_eq!(
            announcement.sender_id(),
            SENDER,
            "送ったのは呼んだ本部のアカウント（下書きを作った本部でなくてよい）"
        );
        assert_eq!(announcement.title().as_str(), "メンテナンスのお知らせ");
        assert_eq!(announcement.body().as_str(), "11/3 は使えません");
        assert_eq!(announcement.sport(), Some(Sport::Swimming));
        assert_eq!(announcement.recipient_count(), 1);
        assert_eq!(announcement.sent_at(), at(30));
        assert_eq!(
            stores.announcements.all(),
            std::slice::from_ref(&announcement)
        );
        assert_eq!(
            stores
                .notifications
                .all()
                .iter()
                .map(Notification::push)
                .collect::<Vec<_>>(),
            [announcement.push_to(SWIMMING_STUDENT)],
            "送るお知らせと同じく対象の生徒にお知らせを作る"
        );
        assert_eq!(stores.push.sent(), [announcement.push_to(SWIMMING_STUDENT)]);
        assert!(drafts.all().is_empty(), "送った下書きは消える");
    }

    #[tokio::test]
    async fn an_empty_title_or_body_is_rejected_and_keeps_the_draft() {
        for (title, body, expected) in [
            (" ", "本文", NotificationError::EmptyAnnouncementTitle),
            ("題", "", NotificationError::EmptyAnnouncementBody),
        ] {
            let stores = Stores::default();
            let drafts =
                InMemoryAnnouncementDraftRepository::with_drafts([draft(title, body, None)]);
            let result = use_case(drafts.clone(), &stores)
                .execute(DRAFT, SENDER)
                .await;
            assert!(
                matches!(&result, Err(AnnouncementDraftError::Invalid(error)) if *error == expected),
                "{title:?} {body:?}: {result:?}"
            );
            assert_eq!(drafts.all(), [draft(title, body, None)], "下書きは残す");
            assert!(stores.announcements.all().is_empty());
            assert!(stores.notifications.all().is_empty());
        }
    }

    #[tokio::test]
    async fn no_students_sends_nothing_and_keeps_the_draft() {
        let stores = Stores::default();
        let drafts = InMemoryAnnouncementDraftRepository::with_drafts([draft(
            "題",
            "本文",
            Some(Sport::Golf),
        )]);
        let result = use_case(drafts.clone(), &stores)
            .execute(DRAFT, SENDER)
            .await;
        assert!(matches!(
            result,
            Err(AnnouncementDraftError::Invalid(
                NotificationError::AnnouncementWithoutRecipients
            ))
        ));
        assert_eq!(drafts.all().len(), 1, "下書きは残す");
        assert!(stores.announcements.all().is_empty());
        assert!(stores.push.sent().is_empty());
    }

    #[tokio::test]
    async fn a_missing_draft_is_not_found_and_sends_nothing() {
        let stores = Stores::default();
        let result = use_case(InMemoryAnnouncementDraftRepository::default(), &stores)
            .execute(DRAFT, SENDER)
            .await;
        assert!(matches!(result, Err(AnnouncementDraftError::DraftNotFound)));
        assert!(stores.announcements.all().is_empty());
    }

    #[tokio::test]
    async fn a_draft_that_cannot_be_deleted_does_not_undo_the_send() {
        let stores = Stores::default();
        let announcement = use_case(UndeletableDrafts(draft("題", "本文", None)), &stores)
            .execute(DRAFT, SENDER)
            .await
            .unwrap();
        assert_eq!(announcement.recipient_count(), 2);
        assert_eq!(stores.announcements.all(), [announcement]);
    }
}
