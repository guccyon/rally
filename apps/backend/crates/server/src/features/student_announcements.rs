//! 本部が見る生徒 1 人の受け取ったお知らせ（`GET /students/{student_id}/announcements`）。お知らせと既読をお知らせの
//! 保存先から、題と取り下げを送った記録の保存先から読み、生徒かはアカウントの保存先から
//! （`ProfileAnnouncementRecipientLookup`）確かめる。

use application::{
    notification::ListStudentAnnouncements, profile::ProfileAnnouncementRecipientLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::student_announcements_router(
        ListStudentAnnouncements::new(
            ports.notifications(),
            ports.announcements(),
            ProfileAnnouncementRecipientLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
