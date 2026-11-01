//! 本部によるコーチの受付の停止・再開と、止めた・再開した記録の照会（`POST /coaches/{coach_id}/suspend`・`/resume`・
//! `GET /coaches/{coach_id}/acceptance-changes`）。コーチがいるかはアカウントの保存先に問い合わせる
//! （`ProfileCoachIntroductionLookup`）。

use application::{
    profile::ProfileCoachIntroductionLookup,
    scheduling::{ListCoachAcceptanceChanges, ResumeCoach, SuspendCoach},
};
use axum::Router;
use infra::MockTokenService;

use crate::ports::Ports;

pub(super) fn router<P: Ports>(ports: &P) -> Router {
    interface::coach_acceptance_router(
        SuspendCoach::new(
            ports.coach_acceptances(),
            ProfileCoachIntroductionLookup::new(ports.accounts()),
            ports.clock(),
        ),
        ResumeCoach::new(
            ports.coach_acceptances(),
            ProfileCoachIntroductionLookup::new(ports.accounts()),
            ports.clock(),
        ),
        ListCoachAcceptanceChanges::new(
            ports.coach_acceptances(),
            ProfileCoachIntroductionLookup::new(ports.accounts()),
        ),
        MockTokenService,
    )
}
