//! 本部からのお知らせ（`POST /announcements`・`GET /announcements`・`POST /announcements/{announcement_id}/withdraw`）。受け取る生徒はアカウントの保存先から読み
//! （`ProfileAnnouncementRecipientLookup`）、生徒ごとにお知らせを残して、受け取り設定に従ってプッシュ通知を送る。

use application::{
    notification::{ListAnnouncements, SendAnnouncement, WithdrawAnnouncement},
    profile::ProfileAnnouncementRecipientLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::announcements_router(
        SendAnnouncement::new(
            ports.announcements(),
            ProfileAnnouncementRecipientLookup::new(ports.accounts()),
            ports.notifications(),
            ports.notification_settings(),
            ports.push(),
            ports.clock(),
        ),
        ListAnnouncements::new(ports.announcements(), ports.notifications()),
        WithdrawAnnouncement::new(ports.announcements(), ports.notifications(), ports.clock()),
        MockTokenService,
    )
}
