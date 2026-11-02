//! 本部が見る、返信のないまま時間の過ぎたスレッドの一覧（`GET /message-threads/unanswered`）。担当コーチと生徒の
//! 表示名をアカウントの保存先から読んで添える。

use application::{
    messaging::ListUnansweredMessageThreads,
    profile::{ProfileCoachDisplayNameLookup, ProfileStudentDisplayNameLookup},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::unanswered_message_threads_router(
        ListUnansweredMessageThreads::new(
            ports.threads(),
            ProfileCoachDisplayNameLookup::new(ports.accounts()),
            ProfileStudentDisplayNameLookup::new(ports.accounts()),
            ports.clock(),
        ),
        MockTokenService,
    )
}
