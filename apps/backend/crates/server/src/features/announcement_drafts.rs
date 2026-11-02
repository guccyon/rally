//! 本部からのお知らせの下書き（`/announcement-drafts`・`/announcement-drafts/{announcement_draft_id}`・
//! `/announcement-drafts/{announcement_draft_id}/send`）。下書きから送るのは `POST /announcements` と同じ組み立て
//! （受け取る生徒はアカウントの保存先から読む `ProfileAnnouncementRecipientLookup`）で、送ったら下書きを消す。

use application::{
    notification::{
        CreateAnnouncementDraft, DeleteAnnouncementDraft, ListAnnouncementDrafts, SendAnnouncement,
        SendAnnouncementDraft, UpdateAnnouncementDraft,
    },
    profile::ProfileAnnouncementRecipientLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::announcement_drafts_router(
        CreateAnnouncementDraft::new(ports.announcement_drafts(), ports.clock()),
        ListAnnouncementDrafts::new(ports.announcement_drafts()),
        UpdateAnnouncementDraft::new(ports.announcement_drafts(), ports.clock()),
        DeleteAnnouncementDraft::new(ports.announcement_drafts()),
        SendAnnouncementDraft::new(
            ports.announcement_drafts(),
            SendAnnouncement::new(
                ports.announcements(),
                ProfileAnnouncementRecipientLookup::new(ports.accounts()),
                ports.notifications(),
                ports.notification_settings(),
                ports.push(),
                ports.clock(),
            ),
        ),
        MockTokenService,
    )
}
