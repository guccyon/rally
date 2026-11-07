use std::collections::{BTreeMap, BTreeSet};

use domain::scheduling::BookingId;

use super::{CoachLeaveRepository, SlotCoachAccountLookup};
use crate::{
    Unavailable,
    notification::{CoachLeaveNotice, CoachLeaveNoticeLookup},
};

/// [`CoachLeaveNoticeLookup`] の実装（scheduling が notification に答える）。お休みの記録（[`CoachLeaveRepository`]）から
/// キャンセルした予約の写し（日時）と一言を、profile（[`SlotCoachAccountLookup`]）からコーチの表示名を読む。
pub struct SchedulingCoachLeaveNoticeLookup<L, A> {
    leaves: L,
    coaches: A,
}

impl<L, A> SchedulingCoachLeaveNoticeLookup<L, A> {
    pub fn new(leaves: L, coaches: A) -> Self {
        Self { leaves, coaches }
    }
}

impl<L, A> CoachLeaveNoticeLookup for SchedulingCoachLeaveNoticeLookup<L, A>
where
    L: CoachLeaveRepository,
    A: SlotCoachAccountLookup,
{
    async fn find_coach_leave_notices(
        &self,
        booking_ids: &BTreeSet<BookingId>,
    ) -> Result<BTreeMap<BookingId, CoachLeaveNotice>, Unavailable> {
        if booking_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let leaves = self.leaves.coach_leaves_of_bookings(booking_ids).await?;
        let coach_ids: BTreeSet<_> = leaves.iter().map(|leave| leave.coach_id()).collect();
        let coaches = if coach_ids.is_empty() {
            BTreeMap::new()
        } else {
            self.coaches.find_slot_coach_accounts(&coach_ids).await?
        };
        Ok(leaves
            .iter()
            .flat_map(|leave| {
                let coach_display_name = coaches
                    .get(&leave.coach_id())
                    .map(|coach| coach.display_name().clone());
                leave
                    .cancelled_bookings()
                    .iter()
                    .filter(|booking| booking_ids.contains(&booking.booking_id()))
                    .map(move |booking| {
                        (
                            booking.booking_id(),
                            CoachLeaveNotice {
                                coach_display_name: coach_display_name.clone(),
                                starts_at: booking.starts_at(),
                                ends_at: booking.ends_at(),
                                note: leave.note().clone(),
                            },
                        )
                    })
            })
            .collect())
    }
}
