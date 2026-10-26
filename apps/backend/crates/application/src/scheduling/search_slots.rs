use std::{num::NonZeroUsize, time::SystemTime};

use domain::identity::{AccountId, Sport};

use super::{SlotCriteria, SlotCursor, SlotPage, SlotRepository};
use crate::{Unavailable, clock::Clock};

/// スロットの検索の入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSlotsQuery {
    pub sport: Sport,
    /// あればこのコーチの枠だけ
    pub coach_id: Option<AccountId>,
    /// この時刻以降に始まる枠（含む）。省略時は現在時刻
    pub starts_after: Option<SystemTime>,
    /// あればこの時刻より前に始まる枠（含まない）
    pub starts_before: Option<SystemTime>,
    /// 1 ページの最大件数
    pub limit: NonZeroUsize,
    /// 前のページの `next_cursor`
    pub cursor: Option<SlotCursor>,
}

/// スロットの検索のユースケース（student-app の `searchSlots`）。
///
/// 公開中でまだ予約されていない枠を、スポーツ・コーチ・開始時刻の期間で絞り、
/// （開始時刻, スロット ID）の昇順でページに分けて返す。
pub struct SearchSlots<S, C> {
    slots: S,
    clock: C,
}

impl<S, C> SearchSlots<S, C>
where
    S: SlotRepository,
    C: Clock,
{
    pub fn new(slots: S, clock: C) -> Self {
        Self { slots, clock }
    }

    pub async fn execute(&self, query: SearchSlotsQuery) -> Result<SlotPage, Unavailable> {
        let limit = query.limit.get();
        // 1 件多く読み、あふれたら次のページがある
        let mut items = self
            .slots
            .search(&SlotCriteria {
                sport: query.sport,
                coach_id: query.coach_id,
                starts_after: query.starts_after.unwrap_or_else(|| self.clock.now()),
                starts_before: query.starts_before,
                after: query.cursor,
                limit: limit + 1,
            })
            .await?;
        let next_cursor = if items.len() > limit {
            items.truncate(limit);
            items.last().map(SlotCursor::after)
        } else {
            None
        };
        Ok(SlotPage { items, next_cursor })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::scheduling::{Slot, SlotId, SlotStatus};

    use super::*;
    use crate::in_memory::{FixedClock, InMemorySlotRepository};

    const COACH: AccountId = AccountId::from_u128(7);
    const OTHER_COACH: AccountId = AccountId::from_u128(8);

    fn at(hour: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(hour * 3600)
    }

    fn slot(id: u128, coach: AccountId, sport: Sport, hour: u64, status: SlotStatus) -> Slot {
        Slot::restore(
            SlotId::from_u128(id),
            coach,
            sport,
            at(hour),
            at(hour + 1),
            status,
        )
        .unwrap()
    }

    fn query(limit: usize) -> SearchSlotsQuery {
        SearchSlotsQuery {
            sport: Sport::Tennis,
            coach_id: None,
            starts_after: None,
            starts_before: None,
            limit: NonZeroUsize::new(limit).unwrap(),
            cursor: None,
        }
    }

    fn ids(page: &SlotPage) -> Vec<u128> {
        page.items.iter().map(|s| s.id().as_u128()).collect()
    }

    /// 現在時刻は 10 時。テニスの公開中の枠は 11・12・13・14 時（14 時は別のコーチ）。
    fn use_case() -> SearchSlots<InMemorySlotRepository, FixedClock> {
        let slots = InMemorySlotRepository::with_slots([
            slot(1, COACH, Sport::Tennis, 9, SlotStatus::Open),
            slot(2, COACH, Sport::Tennis, 13, SlotStatus::Open),
            slot(3, COACH, Sport::Tennis, 11, SlotStatus::Open),
            slot(4, COACH, Sport::Soccer, 12, SlotStatus::Open),
            slot(5, COACH, Sport::Tennis, 12, SlotStatus::Open),
            slot(6, COACH, Sport::Tennis, 15, SlotStatus::Booked),
            slot(7, COACH, Sport::Tennis, 16, SlotStatus::Closed),
            slot(8, OTHER_COACH, Sport::Tennis, 14, SlotStatus::Open),
        ]);
        SearchSlots::new(slots, FixedClock(at(10)))
    }

    #[tokio::test]
    async fn returns_open_slots_of_the_sport_from_now_in_start_order() {
        let page = use_case().execute(query(20)).await.unwrap();
        assert_eq!(ids(&page), [3, 5, 2, 8]);
        assert_eq!(page.next_cursor, None);
    }

    #[tokio::test]
    async fn filters_by_coach_and_period() {
        let use_case = use_case();
        let page = use_case
            .execute(SearchSlotsQuery {
                coach_id: Some(OTHER_COACH),
                ..query(20)
            })
            .await
            .unwrap();
        assert_eq!(ids(&page), [8]);

        let page = use_case
            .execute(SearchSlotsQuery {
                starts_after: Some(at(9)),
                starts_before: Some(at(13)),
                ..query(20)
            })
            .await
            .unwrap();
        assert_eq!(ids(&page), [1, 3, 5], "開始は含み、終わりは含まない");

        let page = use_case
            .execute(SearchSlotsQuery {
                sport: Sport::Soccer,
                ..query(20)
            })
            .await
            .unwrap();
        assert_eq!(ids(&page), [4]);
    }

    #[tokio::test]
    async fn pages_through_results_with_the_cursor() {
        let use_case = use_case();
        let mut cursor = None;
        let mut pages = Vec::new();
        loop {
            let page = use_case
                .execute(SearchSlotsQuery { cursor, ..query(3) })
                .await
                .unwrap();
            pages.push(ids(&page));
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(pages, [vec![3, 5, 2], vec![8]]);

        // ちょうど割り切れるときは、最後のページに次のカーソルがない
        let page = use_case.execute(query(4)).await.unwrap();
        assert_eq!((ids(&page).len(), page.next_cursor), (4, None));
    }

    #[tokio::test]
    async fn slots_starting_at_the_same_time_are_ordered_by_id() {
        let slots = InMemorySlotRepository::with_slots([
            slot(9, OTHER_COACH, Sport::Tennis, 11, SlotStatus::Open),
            slot(3, COACH, Sport::Tennis, 11, SlotStatus::Open),
        ]);
        let use_case = SearchSlots::new(slots, FixedClock(at(10)));
        let first = use_case.execute(query(1)).await.unwrap();
        assert_eq!(ids(&first), [3]);
        let second = use_case
            .execute(SearchSlotsQuery {
                cursor: first.next_cursor,
                ..query(1)
            })
            .await
            .unwrap();
        assert_eq!(ids(&second), [9]);
        assert_eq!(second.next_cursor, None);
    }
}
