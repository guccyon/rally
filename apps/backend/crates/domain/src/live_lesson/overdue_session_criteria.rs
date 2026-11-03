use std::time::{Duration, SystemTime};

use super::LiveLessonSession;
use crate::{
    DomainError,
    catalog::Sport,
    identity::{Account, AccountId},
    profile::CoachSportFilter,
};

/// 本部が見る「終わっていないまま時間の過ぎた」通話セッションの基準（admin-web の `listOverdueLiveLessonSessions`。
/// 既定・範囲・境界の正本は `specs/contracts/api/admin-web/openapi.yaml` の `listOverdueLiveLessonSessions`）。
///
/// 終了の記録がなく（[`super::SessionStatus::InProgress`]）、照会した時刻に開始から [`Self::hours`] 時間以上たった
/// セッションを「時間の過ぎた」セッションとする。時間は [`Self::MIN_HOURS`]〜[`Self::MAX_HOURS`] 時間で、省いたときは
/// [`Self::DEFAULT_HOURS`] 時間（[`Default`]）。既定と範囲はこの型の 1 か所だけに置く。任意のコーチ [`Self::coach_id`]
/// で絞れ（[`Self::with_coach_id`]。省けばすべてのコーチ）、時間と組み合わせても時間の規則と並べ方は変わらない。
/// 任意の競技 [`Self::sport`] でも絞れる（[`Self::with_sport`]。省けばすべての競技）。セッションの競技はセッションの
/// コーチの競技で、コーチの提供スポーツに含むかの判定は [`CoachSportFilter`] の 1 か所（[`Self::matches_coach_sport`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverdueSessionCriteria {
    hours: u16,
    coach_id: Option<AccountId>,
    coach_sport: CoachSportFilter,
}

impl OverdueSessionCriteria {
    /// 時間を省いたときの、開始からの時間（3 時間）
    pub const DEFAULT_HOURS: u16 = 3;
    /// 指定できる時間の下限（1 時間）
    pub const MIN_HOURS: u16 = 1;
    /// 指定できる時間の上限（7 日 = 168 時間）
    pub const MAX_HOURS: u16 = 168;

    /// 開始から `hours` 時間以上たったセッションを見る基準。[`Self::MIN_HOURS`]〜[`Self::MAX_HOURS`] の外なら
    /// [`DomainError::OverdueSessionHoursOutOfRange`]。
    pub fn new(hours: u16) -> Result<Self, DomainError> {
        if !(Self::MIN_HOURS..=Self::MAX_HOURS).contains(&hours) {
            return Err(DomainError::OverdueSessionHoursOutOfRange {
                min: Self::MIN_HOURS,
                max: Self::MAX_HOURS,
            });
        }
        Ok(Self {
            hours,
            coach_id: None,
            coach_sport: CoachSportFilter::new(None),
        })
    }

    /// `coach_id` があればそのコーチのセッションだけを選ぶ基準（`None` ならすべてのコーチ）。時間は変えない。
    pub const fn with_coach_id(self, coach_id: Option<AccountId>) -> Self {
        Self { coach_id, ..self }
    }

    /// `sport` があればその競技のコーチのセッションだけを選ぶ基準（`None` ならすべての競技）。時間とコーチは変えない。
    pub const fn with_sport(self, sport: Option<Sport>) -> Self {
        Self {
            coach_sport: CoachSportFilter::new(sport),
            ..self
        }
    }

    /// 開始からの時間（時間単位）。
    pub const fn hours(&self) -> u16 {
        self.hours
    }

    /// 絞り込むコーチ。`None` ならすべてのコーチ。
    pub const fn coach_id(&self) -> Option<AccountId> {
        self.coach_id
    }

    /// 絞り込む競技。`None` ならすべての競技。
    pub const fn sport(&self) -> Option<Sport> {
        self.coach_sport.sport()
    }

    /// セッションのコーチのアカウント `coach`（引けなければ `None`）が競技の指定に合うか。競技の指定がなければ常に合う。
    /// 指定があれば、コーチの役割のアカウントで提供スポーツに競技を含むときだけ合う（判定は [`CoachSportFilter`]。
    /// `listCoaches` の `sport` と同じ）。アカウントの引けないコーチは合わない。
    pub fn matches_coach_sport(&self, coach: Option<&Account>) -> bool {
        match self.sport() {
            None => true,
            Some(_) => coach.is_some_and(|coach| self.coach_sport.matches(coach)),
        }
    }

    /// 開始からの時間の長さ。
    pub const fn threshold(&self) -> Duration {
        Duration::from_secs(self.hours as u64 * 3600)
    }

    /// `now` に照会したとき、この時刻（含む）より前に開始したセッションが対象になる。`now` から時間を引けない
    /// （時刻の範囲の外）ときは UNIX 時刻の起点。
    pub fn started_at_or_before(&self, now: SystemTime) -> SystemTime {
        now.checked_sub(self.threshold())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    }

    /// `session` が `now` に照会したときの、終わっていないまま時間の過ぎたセッションで、コーチの指定があればそのコーチの
    /// セッションか。
    pub fn matches(&self, session: &LiveLessonSession, now: SystemTime) -> bool {
        session.ended_at().is_none()
            && session.started_at() <= self.started_at_or_before(now)
            && self
                .coach_id
                .is_none_or(|coach_id| session.coach_id() == coach_id)
    }

    /// `sessions` から `now` に照会したときの時間の過ぎたセッションだけを選び、開始の古い順（同じ時刻はセッションの
    /// ID の順）に並べる。
    pub fn select(
        &self,
        sessions: Vec<LiveLessonSession>,
        now: SystemTime,
    ) -> Vec<LiveLessonSession> {
        let mut overdue: Vec<LiveLessonSession> = sessions
            .into_iter()
            .filter(|session| self.matches(session, now))
            .collect();
        overdue.sort_by_key(|session| (session.started_at(), session.id()));
        overdue
    }
}

impl Default for OverdueSessionCriteria {
    fn default() -> Self {
        Self {
            hours: Self::DEFAULT_HOURS,
            coach_id: None,
            coach_sport: CoachSportFilter::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::{Email, Role},
        live_lesson::{ConnectionQuality, SessionId},
        profile::DisplayName,
        scheduling::BookingId,
    };

    const HOUR: Duration = Duration::from_secs(3600);
    const COACH: AccountId = AccountId::from_u128(2);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn now() -> SystemTime {
        // 2026-11-01T00:00:00Z
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_491_200)
    }

    fn session(
        id: u128,
        started_at: SystemTime,
        ended_at: Option<SystemTime>,
    ) -> LiveLessonSession {
        coach_session(id, COACH, started_at, ended_at)
    }

    fn coach_session(
        id: u128,
        coach: AccountId,
        started_at: SystemTime,
        ended_at: Option<SystemTime>,
    ) -> LiveLessonSession {
        LiveLessonSession::restore(
            SessionId::from_u128(id),
            BookingId::from_u128(100 + id),
            AccountId::from_u128(1),
            coach,
            vec![],
            started_at,
            ended_at.map(|ended_at| (ended_at, ConnectionQuality::new(0.0, 0, 0, 0).unwrap())),
        )
        .unwrap()
    }

    fn ids(sessions: &[LiveLessonSession]) -> Vec<u128> {
        sessions.iter().map(|s| s.id().as_u128()).collect()
    }

    #[test]
    fn the_default_is_three_hours() {
        let criteria = OverdueSessionCriteria::default();
        assert_eq!(criteria.hours(), 3);
        assert_eq!(criteria.threshold(), 3 * HOUR);
        assert_eq!(criteria.started_at_or_before(now()), now() - 3 * HOUR);
        assert_eq!(criteria.coach_id(), None, "既定はすべてのコーチ");
        assert_eq!(criteria.sport(), None, "既定はすべての競技");
    }

    #[test]
    fn accepts_one_to_one_hundred_sixty_eight_hours() {
        for hours in [1, 3, 24, 168] {
            assert_eq!(OverdueSessionCriteria::new(hours).unwrap().hours(), hours);
        }
        for hours in [0, 169, u16::MAX] {
            assert_eq!(
                OverdueSessionCriteria::new(hours),
                Err(DomainError::OverdueSessionHoursOutOfRange { min: 1, max: 168 }),
                "{hours}"
            );
        }
    }

    #[test]
    fn in_progress_sessions_started_at_least_the_hours_ago_match() {
        let criteria = OverdueSessionCriteria::default();
        // ちょうど 3 時間前に開始したものは含む。3 時間に 1 秒足りないものは含まない
        assert!(criteria.matches(&session(1, now() - 3 * HOUR, None), now()));
        assert!(criteria.matches(&session(1, now() - 30 * HOUR, None), now()));
        assert!(!criteria.matches(
            &session(1, now() - 3 * HOUR + Duration::from_secs(1), None),
            now()
        ));
        // 終了したセッションは、どれだけ前に開始していても含まない
        assert!(!criteria.matches(
            &session(1, now() - 30 * HOUR, Some(now() - 29 * HOUR)),
            now()
        ));
    }

    #[test]
    fn the_hours_change_the_threshold() {
        let criteria = OverdueSessionCriteria::new(1).unwrap();
        assert!(criteria.matches(&session(1, now() - HOUR, None), now()));
        assert!(
            !OverdueSessionCriteria::new(2)
                .unwrap()
                .matches(&session(1, now() - HOUR, None), now())
        );
    }

    #[test]
    fn selects_overdue_sessions_oldest_first() {
        let sessions = vec![
            session(1, now() - 4 * HOUR, None),
            session(2, now() - 10 * HOUR, Some(now() - 9 * HOUR)),
            session(3, now() - HOUR, None),
            session(5, now() - 8 * HOUR, None),
            session(4, now() - 8 * HOUR, None),
        ];
        assert_eq!(
            ids(&OverdueSessionCriteria::default().select(sessions, now())),
            [4, 5, 1],
            "同じ開始時刻はセッションの ID の順"
        );
    }

    #[test]
    fn with_coach_id_keeps_the_hours() {
        let criteria = OverdueSessionCriteria::new(24)
            .unwrap()
            .with_coach_id(Some(COACH));
        assert_eq!((criteria.hours(), criteria.coach_id()), (24, Some(COACH)));
        assert_eq!(
            OverdueSessionCriteria::default().with_coach_id(None),
            OverdueSessionCriteria::default()
        );
    }

    #[test]
    fn the_coach_narrows_the_sessions_with_the_hours() {
        let criteria = OverdueSessionCriteria::default().with_coach_id(Some(COACH));
        assert!(criteria.matches(&coach_session(1, COACH, now() - 3 * HOUR, None), now()));
        // 別のコーチのセッションは、時間が過ぎていても含まない
        assert!(!criteria.matches(
            &coach_session(1, OTHER_COACH, now() - 30 * HOUR, None),
            now()
        ));
        // そのコーチのセッションでも、時間に足りないもの・終了したものは含まない
        assert!(!criteria.matches(&coach_session(1, COACH, now() - HOUR, None), now()));
        assert!(!criteria.matches(
            &coach_session(1, COACH, now() - 30 * HOUR, Some(now() - 29 * HOUR)),
            now()
        ));

        let sessions = vec![
            coach_session(1, COACH, now() - 4 * HOUR, None),
            coach_session(2, OTHER_COACH, now() - 10 * HOUR, None),
            coach_session(3, COACH, now() - 8 * HOUR, None),
            coach_session(4, COACH, now() - HOUR, None),
        ];
        assert_eq!(
            ids(&OverdueSessionCriteria::new(5)
                .unwrap()
                .with_coach_id(Some(COACH))
                .select(sessions.clone(), now())),
            [3]
        );
        assert_eq!(
            ids(&criteria.select(sessions.clone(), now())),
            [3, 1],
            "開始の古い順のまま"
        );
        assert_eq!(
            ids(&OverdueSessionCriteria::default()
                .with_coach_id(Some(AccountId::from_u128(99)))
                .select(sessions, now())),
            Vec::<u128>::new(),
            "セッションのないコーチは空"
        );
    }

    fn account(role: Role, sports: &[Sport]) -> Account {
        Account::new(
            COACH,
            Email::new("coach@example.com").unwrap(),
            DisplayName::new("たなか").unwrap(),
            role,
            sports.iter().copied(),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[test]
    fn with_sport_keeps_the_hours_and_the_coach() {
        let criteria = OverdueSessionCriteria::new(24)
            .unwrap()
            .with_coach_id(Some(COACH))
            .with_sport(Some(Sport::Golf));
        assert_eq!(
            (criteria.hours(), criteria.coach_id(), criteria.sport()),
            (24, Some(COACH), Some(Sport::Golf))
        );
        assert_eq!(
            OverdueSessionCriteria::default().with_sport(None),
            OverdueSessionCriteria::default()
        );
    }

    #[test]
    fn without_sport_every_coach_matches() {
        let criteria = OverdueSessionCriteria::default();
        assert!(criteria.matches_coach_sport(Some(&account(Role::Coach, &[Sport::Soccer]))));
        assert!(
            criteria.matches_coach_sport(None),
            "アカウントの引けないコーチも、競技を指定しなければ絞らない"
        );
    }

    #[test]
    fn sport_matches_coaches_offering_it_among_several() {
        let criteria = OverdueSessionCriteria::default().with_sport(Some(Sport::Golf));
        assert!(
            criteria
                .matches_coach_sport(Some(&account(Role::Coach, &[Sport::Tennis, Sport::Golf])))
        );
        assert!(!criteria.matches_coach_sport(Some(&account(Role::Coach, &[Sport::Tennis]))));
        // コーチでないアカウント・引けないコーチは合わない
        assert!(!criteria.matches_coach_sport(Some(&account(Role::Student, &[Sport::Golf]))));
        assert!(!criteria.matches_coach_sport(None));
    }
}
