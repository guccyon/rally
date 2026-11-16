//! KPI の集計が読む行動イベントのポート。

use std::future::Future;

use super::KpiEvent;
use crate::Unavailable;

/// KPI の式（specs/analytics/kpi.yaml）が参照する行動イベントを、イベント名ごとにすべて読むポート。
///
/// 期間では絞らない（式の `exists`・`first` は期間の外のイベントも見る）。順は問わない。知らないイベント名や、
/// 保存先が使えないときは [`Unavailable`] を返す。失敗したイベントを参照する指標だけが出せなくなる。
pub trait KpiEventSource: Send + Sync {
    fn events(&self, name: &str)
    -> impl Future<Output = Result<Vec<KpiEvent>, Unavailable>> + Send;
}
