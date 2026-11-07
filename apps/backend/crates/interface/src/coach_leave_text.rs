//! コーチのお休みで本部が予約をキャンセルしたことの、生徒へのお知らせの文言（題と本文）。お休みにする前に本部が見る見本
//! （admin-web の `getCoachLeavePreview` の `notification_preview`）と、生徒のお知らせの一覧（`listMyNotifications` の
//! `body`）の文言はここだけで決める（画面は組み立てない）。日時は日本時間で「11/10（火）19:00〜19:30」の形
//! （`specs/design/content.yaml`）。

use std::time::SystemTime;

use application::{notification::CoachLeaveNotice, scheduling::calendar_day};
use time::Weekday;

/// お知らせの題。
pub(crate) const COACH_LEAVE_NOTICE_TITLE: &str = "コーチのお休みで予約をキャンセルしました";

/// お知らせの本文（本部の一言を除く）。`coach_display_name` はコーチの表示名（引けなければ `None` で名前を書かない）。
pub(crate) fn coach_leave_notice_body(
    coach_display_name: Option<&str>,
    starts_at: SystemTime,
    ends_at: SystemTime,
) -> String {
    let coach =
        coach_display_name.map_or_else(|| "コーチ".to_string(), |name| format!("{name} コーチ"));
    format!(
        "{coach}のお休みのため、{} の予約を本部でキャンセルしました。今月のビデオ通話の枠は使いません。ほかの枠やコーチを探して予約できます。",
        slot_time_label(starts_at, ends_at)
    )
}

/// 生徒のお知らせの一覧の本文。題と本文を改行でつなぎ（本部からのお知らせと同じ形）、本部の一言があれば改行して後ろに
/// 付ける。お休みの記録が引けなければ（`None`）日時のない本文にする。
pub(crate) fn coach_leave_notification_text(notice: Option<&CoachLeaveNotice>) -> String {
    let Some(notice) = notice else {
        return format!(
            "{COACH_LEAVE_NOTICE_TITLE}\nコーチのお休みのため、予約を本部でキャンセルしました。今月のビデオ通話の枠は使いません。ほかの枠やコーチを探して予約できます。"
        );
    };
    let body = coach_leave_notice_body(
        notice.coach_display_name.as_ref().map(|name| name.as_str()),
        notice.starts_at,
        notice.ends_at,
    );
    if notice.note.is_empty() {
        format!("{COACH_LEAVE_NOTICE_TITLE}\n{body}")
    } else {
        format!(
            "{COACH_LEAVE_NOTICE_TITLE}\n{body}\n{}",
            notice.note.as_str()
        )
    }
}

/// 枠の日時（「11/10（火）19:00〜19:30」。日本時間。時は 0 を詰めない）。
fn slot_time_label(starts_at: SystemTime, ends_at: SystemTime) -> String {
    let (start, end) = (calendar_day::local(starts_at), calendar_day::local(ends_at));
    format!(
        "{}/{}（{}）{}:{:02}〜{}:{:02}",
        u8::from(start.month()),
        start.day(),
        weekday_label(start.weekday()),
        start.hour(),
        start.minute(),
        end.hour(),
        end.minute()
    )
}

const fn weekday_label(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "月",
        Weekday::Tuesday => "火",
        Weekday::Wednesday => "水",
        Weekday::Thursday => "木",
        Weekday::Friday => "金",
        Weekday::Saturday => "土",
        Weekday::Sunday => "日",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use domain::{profile::DisplayName, scheduling::CoachLeaveNote};
    use time::{Date, Month};

    use super::*;

    /// 2026-11-10（火）の `hour` 時 `minute` 分（日本時間）
    fn nov10(hour: u64, minute: u64) -> SystemTime {
        calendar_day::start(Date::from_calendar_date(2026, Month::November, 10).unwrap())
            + Duration::from_secs(hour * 3600 + minute * 60)
    }

    #[test]
    fn the_body_names_the_coach_and_the_slot_time_in_japan() {
        assert_eq!(
            coach_leave_notice_body(Some("山田 健"), nov10(19, 0), nov10(19, 30)),
            "山田 健 コーチのお休みのため、11/10（火）19:00〜19:30 の予約を本部でキャンセルしました。今月のビデオ通話の枠は使いません。ほかの枠やコーチを探して予約できます。"
        );
        assert!(
            coach_leave_notice_body(None, nov10(9, 0), nov10(9, 30))
                .starts_with("コーチのお休みのため、11/10（火）9:00〜9:30 の予約")
        );
    }

    #[test]
    fn the_notification_text_is_the_title_the_body_and_the_note() {
        let notice = CoachLeaveNotice {
            coach_display_name: Some(DisplayName::new("山田 健").unwrap()),
            starts_at: nov10(19, 0),
            ends_at: nov10(19, 30),
            note: CoachLeaveNote::new("ご迷惑をおかけします。").unwrap(),
        };
        let text = coach_leave_notification_text(Some(&notice));
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines[0], COACH_LEAVE_NOTICE_TITLE);
        assert!(lines[1].starts_with("山田 健 コーチのお休みのため、11/10（火）19:00〜19:30"));
        assert_eq!(lines[2], "ご迷惑をおかけします。");

        let without_note = CoachLeaveNotice {
            note: CoachLeaveNote::default(),
            ..notice
        };
        assert_eq!(
            coach_leave_notification_text(Some(&without_note))
                .lines()
                .count(),
            2
        );
        assert!(
            coach_leave_notification_text(None).starts_with(COACH_LEAVE_NOTICE_TITLE),
            "記録が引けなくても題から始める"
        );
    }
}
