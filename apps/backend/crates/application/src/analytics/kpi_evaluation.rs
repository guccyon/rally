//! KPI の式（[`KpiFormula`]）の評価。kpi.yaml の冒頭の書き方の意味に従い、行動イベントの行から値を出す。

use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::{
    KpiAttribute, KpiEvent, KpiFormula, KpiSpec, KpiUnavailableReason, KpiWindowedFormula,
    kpi_formula::{Aggregate, BinaryOp, Expr, Function, Source},
};

/// 値が出せなかったわけ。`Error` は式・イベントの不備（集計の失敗）。
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Failure {
    Reason(KpiUnavailableReason),
    Error(String),
}

impl From<String> for Failure {
    fn from(error: String) -> Self {
        Self::Error(error)
    }
}

/// 1 回の集計（同じ `now` と、読めたイベント）。
pub(super) struct Evaluation<'a> {
    pub(super) spec: &'a KpiSpec,
    /// イベント名ごとのイベント（読めたものだけ）
    pub(super) events: &'a BTreeMap<String, Vec<KpiEvent>>,
    pub(super) now: SystemTime,
}

/// 式の中の値。時刻と長さは秒（時刻は UNIX エポックから）で持つ。
#[derive(Debug, Clone, Copy, PartialEq)]
enum Value<'e> {
    Null,
    Bool(bool),
    Number(f64),
    Time(f64),
    Duration(f64),
    Text(&'e str),
}

fn seconds(time: SystemTime) -> f64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_secs_f64(),
        Err(before) => -before.duration().as_secs_f64(),
    }
}

impl<'e> From<&'e KpiAttribute> for Value<'e> {
    fn from(attribute: &'e KpiAttribute) -> Self {
        match attribute {
            KpiAttribute::Null => Value::Null,
            KpiAttribute::Bool(value) => Value::Bool(*value),
            KpiAttribute::Number(value) => Value::Number(*value),
            KpiAttribute::Time(value) => Value::Time(seconds(*value)),
            KpiAttribute::Text(value) => Value::Text(value),
        }
    }
}

/// 別名に結び付けた行（内側ほど後ろ）。
type Scope<'e> = Vec<(&'e str, &'e str, &'e KpiEvent)>;

const DAY: f64 = 24.0 * 60.0 * 60.0;

impl<'a> Evaluation<'a> {
    /// `windowed` を、`window_on` の時刻が `start` 以上 `end` 未満の母集団に当てる。母集団が `lookahead_days` 日先まで
    /// 見る指標で、期間の母集団がなく、観測を待っている行しかないなら [`KpiUnavailableReason::NotObservableYet`]。
    pub(super) fn period(
        &self,
        windowed: &'a KpiWindowedFormula,
        lookahead_days: u32,
        start: SystemTime,
        end: SystemTime,
    ) -> Result<f64, Failure> {
        let formula = &windowed.formula;
        let (start, end) = (seconds(start), seconds(end));
        let observed_until = seconds(self.now) - f64::from(lookahead_days) * DAY;
        let mut rows = Vec::new();
        let (mut unobserved, mut observed_later) = (false, false);
        for event in self.events(&formula.population.event)? {
            let at = match Value::from(event.attribute(&windowed.window_on)) {
                Value::Time(at) => at,
                Value::Null => continue,
                other => {
                    return Err(Failure::Error(format!(
                        "window_on の {} が時刻でない: {other:?}",
                        windowed.window_on
                    )));
                }
            };
            if !self.matches(&formula.population, event, &mut Vec::new())? {
                continue;
            }
            if at >= start && at < end {
                rows.push(event);
            } else if at >= observed_until {
                unobserved = true;
            } else if at >= end {
                observed_later = true;
            }
        }
        if rows.is_empty() && lookahead_days > 0 && unobserved && !observed_later {
            return Err(Failure::Reason(KpiUnavailableReason::NotObservableYet {
                observe_days: lookahead_days,
            }));
        }
        self.aggregate(formula, &rows, 0)
    }

    fn events(&self, name: &str) -> Result<&'a [KpiEvent], Failure> {
        self.events
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| Failure::Error(format!("{name} のイベントを読めていない")))
    }

    /// `source` の条件を `event` が満たすか（`event` を `source` の別名に結び付けて評価する）。
    fn matches(
        &self,
        source: &'a Source,
        event: &'a KpiEvent,
        scope: &mut Scope<'a>,
    ) -> Result<bool, String> {
        let Some(filter) = &source.filter else {
            return Ok(true);
        };
        scope.push((&source.alias, &source.event, event));
        let result = self.expr(filter, scope).and_then(truthy);
        scope.pop();
        result
    }

    fn aggregate(
        &self,
        formula: &'a KpiFormula,
        rows: &[&'a KpiEvent],
        depth: usize,
    ) -> Result<f64, Failure> {
        if rows.is_empty() {
            return Err(Failure::Reason(KpiUnavailableReason::NoPopulation));
        }
        let alias = formula.population.alias.as_str();
        let event = formula.population.event.as_str();
        let each = |expr: &'a Expr| {
            rows.iter()
                .map(|row| {
                    let mut scope = vec![(alias, event, *row)];
                    self.expr(expr, &mut scope)
                })
                .collect::<Result<Vec<_>, String>>()
        };
        match &formula.aggregate {
            Aggregate::Ratio(condition) => {
                let mut hits = 0usize;
                for value in each(condition)? {
                    if truthy(value)? {
                        hits += 1;
                    }
                }
                Ok(hits as f64 / rows.len() as f64)
            }
            Aggregate::Avg(expr) => {
                let values = numbers(each(expr)?)?;
                if values.is_empty() {
                    return Err(Failure::Reason(KpiUnavailableReason::NoPopulation));
                }
                Ok(values.iter().sum::<f64>() / values.len() as f64)
            }
            Aggregate::Median(expr) => {
                let mut values = numbers(each(expr)?)?;
                if values.is_empty() {
                    return Err(Failure::Reason(KpiUnavailableReason::NoPopulation));
                }
                values.sort_by(f64::total_cmp);
                let middle = values.len() / 2;
                Ok(if values.len() % 2 == 0 {
                    (values[middle - 1] + values[middle]) / 2.0
                } else {
                    values[middle]
                })
            }
            Aggregate::Variance {
                kpi,
                key,
                min_count,
            } => self.variance(formula, rows, kpi, key, *min_count, depth),
        }
    }

    /// `variance(<kpi> by <key> having count >= <min_count>)`。母集団を `key` で分け、行が `min_count` 以上の群ごとに
    /// `kpi` の集計を当て、その値の母分散をとる。`kpi` の母集団は同じイベントで、その条件も群の行に当てる。
    fn variance(
        &self,
        formula: &'a KpiFormula,
        rows: &[&'a KpiEvent],
        kpi: &str,
        key: &'a Expr,
        min_count: u32,
        depth: usize,
    ) -> Result<f64, Failure> {
        if depth > self.spec.kpis.len() {
            return Err(Failure::Error(format!(
                "variance が {kpi} を循環して参照する"
            )));
        }
        let referenced = match self.spec.get(kpi).map(|kpi| &kpi.formula) {
            Some(Ok(windowed)) => &windowed.formula,
            Some(Err(error)) => return Err(Failure::Error(error.clone())),
            None => {
                return Err(Failure::Error(format!("variance が参照する {kpi} がない")));
            }
        };
        if referenced.population.event != formula.population.event {
            return Err(Failure::Error(format!(
                "variance が参照する {kpi} の母集団が {} でない",
                formula.population.event
            )));
        }
        let mut groups: BTreeMap<String, Vec<&'a KpiEvent>> = BTreeMap::new();
        for row in rows {
            let mut scope = vec![(
                formula.population.alias.as_str(),
                formula.population.event.as_str(),
                *row,
            )];
            let group = match self.expr(key, &mut scope)? {
                Value::Null => continue,
                Value::Text(text) => format!("t:{text}"),
                other => format!("{other:?}"),
            };
            groups.entry(group).or_default().push(row);
        }
        let mut values = Vec::new();
        for group in groups.values() {
            if group.len() < min_count as usize {
                continue;
            }
            let mut members = Vec::new();
            for row in group {
                if self.matches(&referenced.population, row, &mut Vec::new())? {
                    members.push(*row);
                }
            }
            match self.aggregate(referenced, &members, depth + 1) {
                Ok(value) => values.push(value),
                Err(Failure::Reason(_)) => {}
                Err(error) => return Err(error),
            }
        }
        if values.is_empty() {
            return Err(Failure::Reason(KpiUnavailableReason::BelowGroupMinimum {
                min_count,
            }));
        }
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        Ok(values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64)
    }

    fn expr(&self, expr: &'a Expr, scope: &mut Scope<'a>) -> Result<Value<'a>, String> {
        Ok(match expr {
            Expr::Number(value) => Value::Number(*value),
            Expr::Bool(value) => Value::Bool(*value),
            Expr::Now => Value::Time(seconds(self.now)),
            Expr::Attribute {
                qualifier,
                attribute,
            } => {
                let (_, _, event) = scope
                    .iter()
                    .rev()
                    .find(|(alias, _, _)| alias == qualifier)
                    .or_else(|| scope.iter().rev().find(|(_, name, _)| name == qualifier))
                    .ok_or_else(|| {
                        format!("{qualifier}.{attribute} の {qualifier} が結び付いていない")
                    })?;
                Value::from(event.attribute(attribute))
            }
            Expr::Not(inner) => match self.expr(inner, scope)? {
                Value::Null => Value::Null,
                value => Value::Bool(!truthy(value)?),
            },
            Expr::Negate(inner) => match self.expr(inner, scope)? {
                Value::Null => Value::Null,
                Value::Number(value) => Value::Number(-value),
                Value::Duration(value) => Value::Duration(-value),
                other => return Err(format!("- を当てられない: {other:?}")),
            },
            Expr::Binary(BinaryOp::And, left, right) => {
                let left = truthy(self.expr(left, scope)?)?;
                Value::Bool(left && truthy(self.expr(right, scope)?)?)
            }
            Expr::Binary(BinaryOp::Or, left, right) => {
                let left = truthy(self.expr(left, scope)?)?;
                Value::Bool(left || truthy(self.expr(right, scope)?)?)
            }
            Expr::Binary(op, left, right) => {
                let left = self.expr(left, scope)?;
                let right = self.expr(right, scope)?;
                binary(*op, left, right)?
            }
            Expr::Exists(source) => {
                for event in self.events(&source.event).map_err(failure_text)? {
                    if self.matches(source, event, scope)? {
                        return Ok(Value::Bool(true));
                    }
                }
                Value::Bool(false)
            }
            Expr::First { value, source } => {
                let mut first: Option<(f64, &'a KpiEvent)> = None;
                for event in self.events(&source.event).map_err(failure_text)? {
                    if !self.matches(source, event, scope)? {
                        continue;
                    }
                    let Value::Time(at) = Value::from(event.attribute("occurred_at")) else {
                        continue;
                    };
                    if first.is_none_or(|(earliest, _)| at < earliest) {
                        first = Some((at, event));
                    }
                }
                match first {
                    None => Value::Null,
                    Some((_, event)) => {
                        scope.push((&source.alias, &source.event, event));
                        let result = self.expr(value, scope);
                        scope.pop();
                        result?
                    }
                }
            }
            Expr::Call(function, args) => {
                let args = args
                    .iter()
                    .map(|arg| self.expr(arg, scope))
                    .collect::<Result<Vec<_>, _>>()?;
                call(*function, &args)?
            }
        })
    }
}

fn failure_text(failure: Failure) -> String {
    match failure {
        Failure::Error(error) => error,
        Failure::Reason(reason) => format!("{reason:?}"),
    }
}

/// 条件の真偽。null は満たさないとみなす。
fn truthy(value: Value<'_>) -> Result<bool, String> {
    match value {
        Value::Bool(value) => Ok(value),
        Value::Null => Ok(false),
        other => Err(format!("真偽でない: {other:?}")),
    }
}

/// `avg`・`median` に渡す数（null の行は除く）。
fn numbers(values: Vec<Value<'_>>) -> Result<Vec<f64>, String> {
    values
        .into_iter()
        .filter_map(|value| match value {
            Value::Null => None,
            Value::Number(number) if number.is_finite() => Some(Ok(number)),
            other => Some(Err(format!("数でない: {other:?}"))),
        })
        .collect()
}

fn binary<'e>(op: BinaryOp, left: Value<'e>, right: Value<'e>) -> Result<Value<'e>, String> {
    use Value::{Bool, Duration as Dur, Null, Number, Text, Time};
    if left == Null || right == Null {
        return Ok(Null);
    }
    let compare = |ordering: Option<std::cmp::Ordering>| -> Result<Value<'e>, String> {
        let ordering = ordering.ok_or_else(|| format!("比べられない: {left:?} と {right:?}"))?;
        Ok(Bool(match op {
            BinaryOp::Eq => ordering.is_eq(),
            BinaryOp::Ne => ordering.is_ne(),
            BinaryOp::Lt => ordering.is_lt(),
            BinaryOp::Le => ordering.is_le(),
            BinaryOp::Gt => ordering.is_gt(),
            BinaryOp::Ge => ordering.is_ge(),
            _ => unreachable!("比較の演算子だけ"),
        }))
    };
    match op {
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            match (left, right) {
                (Number(a), Number(b)) | (Time(a), Time(b)) | (Dur(a), Dur(b)) => {
                    compare(a.partial_cmp(&b))
                }
                (Text(a), Text(b)) if matches!(op, BinaryOp::Eq | BinaryOp::Ne) => {
                    compare(Some(a.cmp(b)))
                }
                (Bool(a), Bool(b)) if matches!(op, BinaryOp::Eq | BinaryOp::Ne) => {
                    compare(Some(a.cmp(&b)))
                }
                _ => compare(None),
            }
        }
        BinaryOp::Add => match (left, right) {
            (Number(a), Number(b)) => Ok(Number(a + b)),
            (Time(a), Dur(b)) | (Dur(b), Time(a)) => Ok(Time(a + b)),
            (Dur(a), Dur(b)) => Ok(Dur(a + b)),
            _ => Err(format!("足せない: {left:?} と {right:?}")),
        },
        BinaryOp::Sub => match (left, right) {
            (Number(a), Number(b)) => Ok(Number(a - b)),
            (Time(a), Dur(b)) => Ok(Time(a - b)),
            (Time(a), Time(b)) | (Dur(a), Dur(b)) => Ok(Dur(a - b)),
            _ => Err(format!("引けない: {left:?} と {right:?}")),
        },
        BinaryOp::Mul => match (left, right) {
            (Number(a), Number(b)) => Ok(Number(a * b)),
            _ => Err(format!("掛けられない: {left:?} と {right:?}")),
        },
        BinaryOp::Div => match (left, right) {
            (Number(_), Number(0.0)) => Ok(Null),
            (Number(a), Number(b)) => Ok(Number(a / b)),
            _ => Err(format!("割れない: {left:?} と {right:?}")),
        },
        BinaryOp::And | BinaryOp::Or => unreachable!("and・or は短絡して評価する"),
    }
}

fn call<'e>(function: Function, args: &[Value<'e>]) -> Result<Value<'e>, String> {
    use Value::{Duration as Dur, Null, Number, Time};
    Ok(match (function, args) {
        (Function::Coalesce, [a, b]) => {
            if *a == Null {
                *b
            } else {
                *a
            }
        }
        (_, args) if args.contains(&Null) => Null,
        (Function::Clamp, [Number(x), Number(low), Number(high)]) => Number(x.max(*low).min(*high)),
        (Function::Max, [Number(a), Number(b)]) => Number(a.max(*b)),
        (Function::HoursBetween, [Time(start), Time(end)]) => Number((end - start) / 3600.0),
        (Function::DaysBetween, [Time(start), Time(end)]) => Number((end - start) / DAY),
        (Function::Days, [Number(n)]) => Dur(n * DAY),
        (function, args) => return Err(format!("{function:?} に当てられない: {args:?}")),
    })
}

/// `time` の `days` 日前。
pub(super) fn days_before(time: SystemTime, days: u32) -> SystemTime {
    time - Duration::from_secs(u64::from(days) * 24 * 60 * 60)
}
