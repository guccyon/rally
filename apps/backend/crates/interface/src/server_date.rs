//! 応答の `Date` ヘッダーを、サーバーの時計（[`Clock`]）で付ける。
//!
//! 画面（admin-web・生徒アプリ）は `Date` からサーバーとのずれを出し、開始 5 分前・開始時刻などの判断をサーバーの時刻に合わせる
//! （rally #1315）。触れる環境（staging）で QA が時計を進めたとき（`CLOCK_OFFSET_SECONDS`）、`Date` が実際の時刻のままだと API は
//! 「始まっている」、画面は「まだ」と食い違う。そこで `Date` をサーバーの時計で付ける（factory-engine の ADR 0055 の決定 2）。
//! 本番は時計を進めないので、付く値は今までと同じ実際の時刻。HTTP の土台（hyper）は `Date` があれば付け直さない。

use std::{sync::LazyLock, time::SystemTime};

use application::clock::Clock;
use axum::{
    Router,
    extract::State,
    http::{HeaderValue, header::DATE},
    middleware::map_response_with_state,
    response::Response,
};
use time::{OffsetDateTime, format_description::BorrowedFormatItem};

/// HTTP の日付の形（RFC 9110 の IMF-fixdate。例 `Sun, 06 Nov 1994 08:49:37 GMT`）。
static IMF_FIXDATE: LazyLock<Vec<BorrowedFormatItem<'static>>> = LazyLock::new(|| {
    time::format_description::parse_borrowed::<2>(
        "[weekday repr:short], [day] [month repr:short] [year] [hour]:[minute]:[second] GMT",
    )
    .expect("IMF-fixdate の形の書き方は固定")
});

/// `router` のすべての応答に、`clock` の時刻で `Date` を付ける。
pub fn with_server_date<C>(router: Router, clock: C) -> Router
where
    C: Clock + Clone + 'static,
{
    router.layer(map_response_with_state(clock, set_date::<C>))
}

/// `at` を HTTP の日付の形にする。
pub fn http_date(at: SystemTime) -> String {
    OffsetDateTime::from(at)
        .format(&IMF_FIXDATE[..])
        .expect("IMF-fixdate の形はどの時刻でも書ける")
}

async fn set_date<C>(State(clock): State<C>, mut response: Response) -> Response
where
    C: Clock + Clone + 'static,
{
    if let Ok(value) = HeaderValue::from_str(&http_date(clock.now())) {
        response.headers_mut().insert(DATE, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    #[test]
    fn formats_as_imf_fixdate() {
        // 784111777 = 1994-11-06T08:49:37Z（RFC 9110 の例）
        let at = UNIX_EPOCH + Duration::from_secs(784_111_777);
        assert_eq!(http_date(at), "Sun, 06 Nov 1994 08:49:37 GMT");
    }

    #[test]
    fn pads_single_digit_days() {
        // 2026-11-01T00:00:05Z
        let at = UNIX_EPOCH + Duration::from_secs(1_793_491_205);
        assert_eq!(http_date(at), "Sun, 01 Nov 2026 00:00:05 GMT");
    }
}
