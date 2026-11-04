use crate::scheduling::CoachAcceptanceStatus;

/// 本部がコーチを受付の状態で絞るときの条件（admin-web の `listCoaches` と `listIncompleteCoachProfiles` の
/// `acceptance_status`）。受付の状態の持ち主は scheduling で、ここは読んだ状態を条件と比べるだけ。
///
/// - 状態 `Some(status)`: 受付の状態が `status` のコーチだけ
/// - `None`（既定）: 絞らない（どの状態も通す）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoachAcceptanceFilter(Option<CoachAcceptanceStatus>);

impl CoachAcceptanceFilter {
    /// 受付の状態 `status` で絞る条件。`None` なら絞らない。
    pub const fn new(status: Option<CoachAcceptanceStatus>) -> Self {
        Self(status)
    }

    /// 絞る受付の状態。絞らないなら `None`。
    pub const fn status(&self) -> Option<CoachAcceptanceStatus> {
        self.0
    }

    /// 受付の状態が `status` のコーチがこの条件に合うか。
    pub fn matches(&self, status: CoachAcceptanceStatus) -> bool {
        self.0.is_none_or(|wanted| wanted == status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_matches_only_the_same_status() {
        let suspended = CoachAcceptanceFilter::new(Some(CoachAcceptanceStatus::Suspended));
        assert!(suspended.matches(CoachAcceptanceStatus::Suspended));
        assert!(!suspended.matches(CoachAcceptanceStatus::Accepting));

        let accepting = CoachAcceptanceFilter::new(Some(CoachAcceptanceStatus::Accepting));
        assert!(accepting.matches(CoachAcceptanceStatus::Accepting));
        assert!(!accepting.matches(CoachAcceptanceStatus::Suspended));
    }

    #[test]
    fn no_status_matches_every_status() {
        let filter = CoachAcceptanceFilter::default();
        assert_eq!(filter, CoachAcceptanceFilter::new(None));
        assert_eq!(filter.status(), None);
        for status in CoachAcceptanceStatus::ALL {
            assert!(filter.matches(status), "{status:?}");
        }
    }
}
