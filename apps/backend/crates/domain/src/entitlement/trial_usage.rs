/// 生徒の体験（月額契約の前に無料で受けられる、通話か動画添削のどちらか 1 回）の使用状況と、その規則。
///
/// 体験を使ったかの判定はここの 1 か所に置く（体験予約の作成・`listMyBookings` の `trial_available`・動画の送付が同じ
/// 規則を使う）。材料は次の 4 つで、どれも照会した事実の有無だけを持つ（読むのは application）。
///
/// - 月額の受講権（動画とチャットの送り放題。[`super::Entitlement::unlimited_video_and_chat`]）がある
/// - キャンセルされていない予約がある（体験予約・通常の予約を問わない）
/// - 完了したレッスン（lesson_completion。同期・非同期のどちらでも）がある
/// - 送った動画（添削待ちも返却済みも含む）がある
///
/// 既定値（[`Default`]）は、どの材料もない（体験を使っていない月額契約の前の生徒）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TrialUsage {
    has_monthly_entitlement: bool,
    has_active_booking: bool,
    has_completed_lesson: bool,
    has_submitted_video: bool,
}

impl TrialUsage {
    /// 月額の受講権（動画とチャットの送り放題）があるか。
    pub const fn with_monthly_entitlement(mut self, has: bool) -> Self {
        self.has_monthly_entitlement = has;
        self
    }

    /// キャンセルされていない予約があるか。
    pub const fn with_active_booking(mut self, has: bool) -> Self {
        self.has_active_booking = has;
        self
    }

    /// 完了したレッスンがあるか。
    pub const fn with_completed_lesson(mut self, has: bool) -> Self {
        self.has_completed_lesson = has;
        self
    }

    /// 送った動画があるか。
    pub const fn with_submitted_video(mut self, has: bool) -> Self {
        self.has_submitted_video = has;
        self
    }

    /// 今体験予約できるか。月額の受講権がなく、キャンセルされていない予約も、完了したレッスンも、送った動画もない
    /// ときだけ true。
    ///
    /// 受け終えた体験予約をあとでキャンセルしても、完了したレッスンが残るので false のまま。受けていない体験予約を
    /// キャンセルしただけなら true。動画を送った生徒は（添削待ちでも）体験を動画添削で使ったので false。月額契約の
    /// 生徒は体験の対象でないので false。
    pub const fn is_trial_available(&self) -> bool {
        !self.has_monthly_entitlement
            && !self.has_active_booking
            && !self.has_completed_lesson
            && !self.has_submitted_video
    }

    /// 動画の添削を依頼できるか。月額の受講権があれば送り放題で常に true。なければ体験として 1 本だけで、完了した
    /// レッスンも送った動画もないときだけ true。
    ///
    /// キャンセルされていない予約は見ない（まだ受けていない体験予約は、添削に切り替えるときに解放する。
    /// [`Self::releases_trial_booking_for_video_review`]）。
    pub const fn may_request_video_review(&self) -> bool {
        self.has_monthly_entitlement || (!self.has_completed_lesson && !self.has_submitted_video)
    }

    /// 動画の添削を体験として受け付けるとき（月額の受講権がなく、依頼できるとき）に true。このとき、まだ受けていない
    /// 体験予約（キャンセルされていない体験予約）は、送付の時点でキャンセル扱いで解放する（体験は通話か動画添削の
    /// どちらか 1 回なので、添削に切り替えた生徒に通話の体験を残さない）。
    pub const fn releases_trial_booking_for_video_review(&self) -> bool {
        !self.has_monthly_entitlement && self.may_request_video_review()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNUSED: TrialUsage = TrialUsage {
        has_monthly_entitlement: false,
        has_active_booking: false,
        has_completed_lesson: false,
        has_submitted_video: false,
    };

    #[test]
    fn default_is_an_unused_trial() {
        assert_eq!(TrialUsage::default(), UNUSED);
        assert!(UNUSED.is_trial_available());
        assert!(UNUSED.may_request_video_review());
        assert!(UNUSED.releases_trial_booking_for_video_review());
    }

    #[test]
    fn any_use_makes_the_trial_unavailable() {
        for usage in [
            UNUSED.with_monthly_entitlement(true),
            UNUSED.with_active_booking(true),
            UNUSED.with_completed_lesson(true),
            UNUSED.with_submitted_video(true),
        ] {
            assert!(!usage.is_trial_available(), "{usage:?}");
        }
    }

    #[test]
    fn video_review_without_monthly_entitlement_is_the_trial_only_once() {
        // まだ受けていない予約があっても、添削に切り替えて送れる（その予約は解放する）
        let booked = UNUSED.with_active_booking(true);
        assert!(booked.may_request_video_review());
        assert!(booked.releases_trial_booking_for_video_review());
        // 受けたレッスンか送った動画があれば体験は使用済み
        for used in [
            UNUSED.with_completed_lesson(true),
            UNUSED.with_submitted_video(true),
            booked.with_completed_lesson(true),
        ] {
            assert!(!used.may_request_video_review(), "{used:?}");
            assert!(!used.releases_trial_booking_for_video_review(), "{used:?}");
        }
    }

    #[test]
    fn monthly_entitlement_sends_videos_without_limit_and_never_books_a_trial() {
        let monthly = UNUSED
            .with_monthly_entitlement(true)
            .with_active_booking(true)
            .with_completed_lesson(true)
            .with_submitted_video(true);
        assert!(monthly.may_request_video_review());
        assert!(!monthly.releases_trial_booking_for_video_review());
        assert!(!monthly.is_trial_available());
        assert!(!UNUSED.with_monthly_entitlement(true).is_trial_available());
    }
}
