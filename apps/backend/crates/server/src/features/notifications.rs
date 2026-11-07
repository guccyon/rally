//! 生徒とコーチのお知らせ（`GET /notifications/me`・`POST /notifications/me/read`）。本部からのお知らせは、送った記録の
//! 保存先から読む。本部による予約のキャンセル（コーチのお休み）の日時と一言は、予約の保存先のお休みの記録と
//! アカウントの保存先から読む（`SchedulingCoachLeaveNoticeLookup`）。

use application::{
    notification::{ListMyNotifications, MarkMyNotificationsRead},
    profile::ProfileCoachAccountLookup,
    scheduling::SchedulingCoachLeaveNoticeLookup,
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::notifications_router(
        ListMyNotifications::new(ports.notifications(), ports.announcements(), notices(ports)),
        MarkMyNotificationsRead::new(
            ports.notifications(),
            ports.announcements(),
            notices(ports),
            ports.clock(),
        ),
        MockTokenService,
    )
}

fn notices<P: Ports>(
    ports: &P,
) -> SchedulingCoachLeaveNoticeLookup<P::Bookings, ProfileCoachAccountLookup<P::Accounts>> {
    SchedulingCoachLeaveNoticeLookup::new(
        ports.bookings(),
        ProfileCoachAccountLookup::new(ports.accounts()),
    )
}
