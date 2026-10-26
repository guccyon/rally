use super::{ContractType, EntitlementId, LiveLessonQuota, SubscriptionId, YearMonth};
use crate::{
    DomainError,
    identity::{AccountId, Sport},
    scheduling::BookingId,
};

/// 受講権（glossary の entitlement）。契約種別と利用権の総称。
///
/// - 月額: 元の契約（[`SubscriptionId`]）を持ち、動画とチャットは送り放題、ビデオ通話は暦月の枠
///   （[`LiveLessonQuota`]）。予約のたびには消費されず、消費されるのは通話の枠だけ
/// - ワンショット: ゴルフだけ。残りの動画添削回数を持つ（Gen1 では型だけ）
///
/// ビデオ通話を予約してよいか（[`Self::may_book_live_lesson`]）と枠の消費・戻し（[`Self::consume_live_lesson`]・
/// [`Self::release_live_lesson`]）の規則はここと [`LiveLessonQuota`] に置く。
///
/// どの契約種別でどのスポーツを結べるかは [`ContractType::covers`] が決める。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entitlement {
    id: EntitlementId,
    student_id: AccountId,
    contract_type: ContractType,
    sport: Sport,
    subscription_id: Option<SubscriptionId>,
    live_lesson_quota: Option<LiveLessonQuota>,
    remaining_video_reviews: Option<u32>,
}

impl Entitlement {
    /// 月額の受講権。`sport` がゴルフなら [`DomainError::SportNotCoveredByContract`]。
    pub fn monthly(
        id: EntitlementId,
        student_id: AccountId,
        sport: Sport,
        subscription_id: SubscriptionId,
        live_lesson_quota: LiveLessonQuota,
    ) -> Result<Self, DomainError> {
        ContractType::Monthly.ensure_covers(sport)?;
        Ok(Self {
            id,
            student_id,
            contract_type: ContractType::Monthly,
            sport,
            subscription_id: Some(subscription_id),
            live_lesson_quota: Some(live_lesson_quota),
            remaining_video_reviews: None,
        })
    }

    /// ワンショットの受講権。`sport` がゴルフ以外なら [`DomainError::SportNotCoveredByContract`]。
    pub fn one_shot(
        id: EntitlementId,
        student_id: AccountId,
        sport: Sport,
        remaining_video_reviews: u32,
    ) -> Result<Self, DomainError> {
        ContractType::OneShot.ensure_covers(sport)?;
        Ok(Self {
            id,
            student_id,
            contract_type: ContractType::OneShot,
            sport,
            subscription_id: None,
            live_lesson_quota: None,
            remaining_video_reviews: Some(remaining_video_reviews),
        })
    }

    /// `month` の時点の受講権。月額の通話の枠は `month` のものになる（月をまたいでいれば使った回数は 0 から）。
    pub fn in_month(mut self, month: YearMonth) -> Self {
        self.live_lesson_quota = self.live_lesson_quota.map(|quota| quota.in_month(month));
        self
    }

    /// `month` に、ビデオ通話をもう 1 回予約してよいか（scheduling が予約の確定の前に問い合わせる判定の規則）。
    ///
    /// 月額は `month` の通話の枠が残っているときだけ。通話の枠を持たない契約種別（ワンショット）には
    /// 暦月の回数の制約がないので常に `true`（不変条件 1 は月額の生徒の規則）。
    pub fn may_book_live_lesson(&self, month: YearMonth) -> bool {
        self.live_lesson_quota
            .as_ref()
            .is_none_or(|quota| quota.has_remaining_in(month))
    }

    /// `booking` の予約で `month` の通話の枠を 1 回使う（月をまたいでいれば `month` の枠を 0 から数えてから）。
    /// すでにその予約で使っていれば何も変えない。枠が残っていなければ [`DomainError::LiveLessonQuotaExceeded`]
    /// で、何も変えない。通話の枠を持たない契約種別（ワンショット）では何もしない。
    pub fn consume_live_lesson(
        &mut self,
        booking: BookingId,
        month: YearMonth,
    ) -> Result<(), DomainError> {
        if let Some(quota) = &self.live_lesson_quota {
            self.live_lesson_quota = Some(quota.clone().in_month(month).consume(booking)?);
        }
        Ok(())
    }

    /// `booking` の予約で `month` に使った通話の枠を戻す。その月にその予約で使っていなければ何も変えない
    /// （月をまたいでいれば `month` の枠を 0 から数えるだけ）。
    pub fn release_live_lesson(&mut self, booking: BookingId, month: YearMonth) {
        self.live_lesson_quota = self
            .live_lesson_quota
            .take()
            .map(|quota| quota.in_month(month).release(booking));
    }

    pub const fn id(&self) -> EntitlementId {
        self.id
    }

    pub const fn student_id(&self) -> AccountId {
        self.student_id
    }

    pub const fn contract_type(&self) -> ContractType {
        self.contract_type
    }

    pub const fn sport(&self) -> Sport {
        self.sport
    }

    /// 元の月額契約（ワンショットは `None`）。
    pub const fn subscription_id(&self) -> Option<SubscriptionId> {
        self.subscription_id
    }

    /// 動画とチャットが送り放題か（月額は `true`）。
    pub const fn unlimited_video_and_chat(&self) -> bool {
        self.contract_type.unlimited_video_and_chat()
    }

    /// ビデオ通話の暦月の枠（月額のみ）。今の月の枠にするには先に [`Self::in_month`] を通す。
    pub const fn live_lesson_quota(&self) -> Option<&LiveLessonQuota> {
        self.live_lesson_quota.as_ref()
    }

    /// 残りの動画添削回数（ワンショットのみ）。
    pub const fn remaining_video_reviews(&self) -> Option<u32> {
        self.remaining_video_reviews
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STUDENT: AccountId = AccountId::from_u128(1);

    fn ym(year: i32, month: u8) -> YearMonth {
        YearMonth::new(year, month).unwrap()
    }

    fn monthly(sport: Sport, quota: LiveLessonQuota) -> Result<Entitlement, DomainError> {
        Entitlement::monthly(
            EntitlementId::from_u128(10),
            STUDENT,
            sport,
            SubscriptionId::from_u128(20),
            quota,
        )
    }

    fn booking(id: u128) -> BookingId {
        BookingId::from_u128(id)
    }

    fn one_shot(sport: Sport) -> Result<Entitlement, DomainError> {
        Entitlement::one_shot(EntitlementId::from_u128(11), STUDENT, sport, 1)
    }

    #[test]
    fn monthly_has_unlimited_video_and_chat_and_a_live_lesson_quota() {
        let quota = LiveLessonQuota::fresh(ym(2026, 10));
        let entitlement = monthly(Sport::Tennis, quota.clone()).unwrap();
        assert_eq!(entitlement.contract_type(), ContractType::Monthly);
        assert!(entitlement.unlimited_video_and_chat());
        assert_eq!(entitlement.live_lesson_quota(), Some(&quota));
        assert_eq!(
            entitlement.subscription_id(),
            Some(SubscriptionId::from_u128(20))
        );
        assert_eq!(entitlement.remaining_video_reviews(), None);
    }

    #[test]
    fn monthly_rejects_golf() {
        assert_eq!(
            monthly(Sport::Golf, LiveLessonQuota::fresh(ym(2026, 10))),
            Err(DomainError::SportNotCoveredByContract {
                contract_type: ContractType::Monthly,
                sport: Sport::Golf,
            })
        );
    }

    #[test]
    fn one_shot_is_golf_only() {
        let entitlement = one_shot(Sport::Golf).unwrap();
        assert_eq!(entitlement.contract_type(), ContractType::OneShot);
        assert!(!entitlement.unlimited_video_and_chat());
        assert_eq!(entitlement.live_lesson_quota(), None);
        assert_eq!(entitlement.subscription_id(), None);
        assert_eq!(entitlement.remaining_video_reviews(), Some(1));
    }

    #[test]
    fn one_shot_rejects_sports_other_than_golf() {
        for sport in Sport::ALL.into_iter().filter(|s| *s != Sport::Golf) {
            assert_eq!(
                one_shot(sport),
                Err(DomainError::SportNotCoveredByContract {
                    contract_type: ContractType::OneShot,
                    sport,
                }),
                "{sport:?}"
            );
        }
    }

    #[test]
    fn crossing_the_month_resets_the_live_lesson_quota() {
        let october = LiveLessonQuota::restore(ym(2026, 10), [booking(1), booking(2)]).unwrap();
        let entitlement = monthly(Sport::Tennis, october.clone()).unwrap();

        assert_eq!(
            entitlement
                .clone()
                .in_month(ym(2026, 10))
                .live_lesson_quota(),
            Some(&october),
            "同じ月は使った回数のまま"
        );
        let november = entitlement
            .in_month(ym(2026, 11))
            .live_lesson_quota()
            .cloned()
            .unwrap();
        assert_eq!(november.month(), ym(2026, 11));
        assert_eq!((november.used(), november.remaining()), (0, 2));
    }

    #[test]
    fn one_shot_has_no_quota_in_any_month() {
        let entitlement = one_shot(Sport::Golf).unwrap().in_month(ym(2026, 11));
        assert_eq!(entitlement.live_lesson_quota(), None);
    }

    #[test]
    fn monthly_may_book_two_live_lessons_a_month_and_the_third_is_rejected() {
        let october = ym(2026, 10);
        let mut entitlement = monthly(Sport::Tennis, LiveLessonQuota::fresh(october)).unwrap();
        for id in [1, 2] {
            assert!(entitlement.may_book_live_lesson(october));
            entitlement
                .consume_live_lesson(booking(id), october)
                .unwrap();
        }
        assert!(
            !entitlement.may_book_live_lesson(october),
            "3 回目は予約できない"
        );
        let before = entitlement.clone();
        assert_eq!(
            entitlement.consume_live_lesson(booking(3), october),
            Err(DomainError::LiveLessonQuotaExceeded)
        );
        assert_eq!(entitlement, before, "拒否しても何も変えない");

        // 翌月は再び予約できる
        let november = ym(2026, 11);
        assert!(entitlement.may_book_live_lesson(november));
        entitlement
            .consume_live_lesson(booking(3), november)
            .unwrap();
        let quota = entitlement.live_lesson_quota().unwrap();
        assert_eq!((quota.month(), quota.used()), (november, 1));
    }

    #[test]
    fn releasing_a_live_lesson_returns_the_quota_of_the_month() {
        let october = ym(2026, 10);
        let mut entitlement = monthly(
            Sport::Tennis,
            LiveLessonQuota::restore(october, [booking(1), booking(2)]).unwrap(),
        )
        .unwrap();
        entitlement.release_live_lesson(booking(1), october);
        assert_eq!(entitlement.live_lesson_quota().unwrap().used(), 1);
        assert!(entitlement.may_book_live_lesson(october));

        // 月をまたいでから戻しても、翌月の枠は 0 から数えるだけ
        entitlement.release_live_lesson(booking(2), ym(2026, 11));
        assert_eq!(
            entitlement.live_lesson_quota(),
            Some(&LiveLessonQuota::fresh(ym(2026, 11)))
        );
    }

    #[test]
    fn one_shot_has_no_monthly_limit_and_consumes_nothing() {
        let mut entitlement = one_shot(Sport::Golf).unwrap();
        let october = ym(2026, 10);
        assert!(entitlement.may_book_live_lesson(october));
        entitlement
            .consume_live_lesson(booking(1), october)
            .unwrap();
        entitlement.release_live_lesson(booking(1), october);
        assert_eq!(entitlement, one_shot(Sport::Golf).unwrap());
    }
}
