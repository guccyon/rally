//! 本部が見るコーチのお休みの期間の予約と枠と、お休みにする操作（`GET /coaches/{coach_id}/leave-preview`・
//! `POST /coaches/{coach_id}/leaves`）。お休みの記録は予約の保存先が持ち、コーチ・生徒の表示名はアカウントの保存先、
//! 予約のない枠のメニューは catalog から読む。お休みでキャンセルした予約は境界間イベントの発行先へ `BookingCancelled` を出す
//! （entitlement が枠を戻し、notification が生徒に知らせる）。

use application::{
    catalog::CatalogLessonMenuLookup,
    profile::{ProfileCoachAccountLookup, ProfileStudentDisplayNameLookup},
    scheduling::{PreviewCoachLeave, StartCoachLeave},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coach_leaves_router(
        PreviewCoachLeave::new(
            ports.bookings(),
            ProfileCoachAccountLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            CatalogLessonMenuLookup::new(ports.lesson_menus()),
            ports.clock(),
        ),
        StartCoachLeave::new(
            ports.bookings(),
            ProfileCoachAccountLookup::new(ports.accounts()),
            ports.events(),
            ports.clock(),
        ),
        MockTokenService,
    )
}
