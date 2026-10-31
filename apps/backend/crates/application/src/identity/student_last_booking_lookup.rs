use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    time::SystemTime,
};

use domain::identity::AccountId;

use crate::Unavailable;

/// identity が scheduling に生徒が最後に予約した時刻を問い合わせるポート（本部が見る生徒の一覧に添える）。
///
/// 予約の持ち主は scheduling で、identity は読むだけ。実装は
/// [`crate::scheduling::SchedulingStudentLastBookingLookup`]。
pub trait StudentLastBookingLookup: Send + Sync {
    /// `student_ids` の生徒それぞれが最後に予約した時刻（予約を作った時刻の最も新しいもの。予約の状態は問わない）。
    /// 予約がない生徒は結果に含めない。
    fn find_last_booked_at(
        &self,
        student_ids: &BTreeSet<AccountId>,
    ) -> impl Future<Output = Result<BTreeMap<AccountId, SystemTime>, Unavailable>> + Send;
}
