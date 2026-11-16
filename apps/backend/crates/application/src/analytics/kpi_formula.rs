//! KPI の式（specs/analytics/kpi.yaml の `formula`）の構文木と、文字列からの組み立て。
//!
//! 書き方は kpi.yaml の冒頭のコメントが正。ここは書き方を読むだけで、個々の KPI の式は持たない（式は kpi.yaml の
//! 1 か所にだけ置く）。評価は [`super::kpi_evaluation`]。

use std::collections::BTreeSet;

/// 1 つの KPI の式。`from <イベント> as <別名> [where <条件>]` の母集団と、その母集団に当てる集計（`ratio`・`avg`・
/// `median`・`variance`）。
#[derive(Debug, Clone, PartialEq)]
pub struct KpiFormula {
    pub(super) population: Source,
    pub(super) aggregate: Aggregate,
}

/// イベントの集まり（`<イベント> as <別名> [where <条件>]`）。母集団と、`exists`・`first` の中で使う。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Source {
    pub(super) event: String,
    pub(super) alias: String,
    pub(super) filter: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Aggregate {
    Ratio(Expr),
    Avg(Expr),
    Median(Expr),
    /// `variance(<KPI の id> by <キー> having count >= <min_count>)`
    Variance {
        kpi: String,
        key: Expr,
        min_count: u32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Expr {
    Number(f64),
    Bool(bool),
    Now,
    /// `<別名>.<属性>` か `<イベント>.<属性>`
    Attribute {
        qualifier: String,
        attribute: String,
    },
    Not(Box<Expr>),
    Negate(Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Exists(Box<Source>),
    /// `first(<値> for <イベント> as <別名> where <条件>)`
    First {
        value: Box<Expr>,
        source: Box<Source>,
    },
    Call(Function, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BinaryOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Function {
    Clamp,
    Max,
    Coalesce,
    HoursBetween,
    DaysBetween,
    Days,
}

impl Function {
    fn from_name(name: &str) -> Option<(Self, usize)> {
        Some(match name {
            "clamp" => (Self::Clamp, 3),
            "max" => (Self::Max, 2),
            "coalesce" => (Self::Coalesce, 2),
            "hours_between" => (Self::HoursBetween, 2),
            "days_between" => (Self::DaysBetween, 2),
            "days" => (Self::Days, 1),
            _ => return None,
        })
    }
}

/// 名前として使えない語（kpi.yaml の `vocabulary` のうち、式の区切りになるもの）。
const KEYWORDS: [&str; 14] = [
    "from", "as", "where", "and", "or", "not", "for", "by", "having", "count", "ratio", "avg",
    "median", "variance",
];

impl KpiFormula {
    /// `text`（kpi.yaml の `formula`）を読む。書き方に合わなければ理由の文。
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut parser = Parser {
            tokens: tokenize(text)?,
            position: 0,
        };
        parser.keyword("from")?;
        let population = parser.source()?;
        let aggregate = parser.aggregate()?;
        match parser.peek() {
            None => Ok(Self {
                population,
                aggregate,
            }),
            Some(token) => Err(format!("集計のあとに余計な {token:?} がある")),
        }
    }

    /// 母集団のイベント名。
    pub fn population_event(&self) -> &str {
        &self.population.event
    }

    /// 母集団の別名。
    pub fn population_alias(&self) -> &str {
        &self.population.alias
    }

    /// 集計が `variance` なら、群ごとに計算する KPI の id。
    pub fn referenced_kpi(&self) -> Option<&str> {
        match &self.aggregate {
            Aggregate::Variance { kpi, .. } => Some(kpi),
            _ => None,
        }
    }

    /// 式が参照するイベント名（母集団と `exists`・`first` の中。`variance` が参照する KPI の分は含まない）。
    pub fn events(&self) -> BTreeSet<String> {
        let mut events = BTreeSet::new();
        self.population.collect_events(&mut events);
        match &self.aggregate {
            Aggregate::Ratio(expr) | Aggregate::Avg(expr) | Aggregate::Median(expr) => {
                expr.collect_events(&mut events)
            }
            Aggregate::Variance { key, .. } => key.collect_events(&mut events),
        }
        events
    }

    /// 母集団の行の `attribute`（kpi.yaml の `window_on` の属性）から先を見る日数の最大（`<別名>.<attribute> + days(n)`
    /// の n）。その日数が過ぎるまで、行の観測は終わっていない。先を見なければ 0。
    pub fn lookahead_days(&self, attribute: &str) -> u32 {
        let mut days = 0;
        self.population
            .collect_lookahead(&self.population.alias, attribute, &mut days);
        if let Aggregate::Ratio(expr) | Aggregate::Avg(expr) | Aggregate::Median(expr) =
            &self.aggregate
        {
            expr.collect_lookahead(&self.population.alias, attribute, &mut days);
        }
        days
    }
}

impl Source {
    fn collect_events(&self, events: &mut BTreeSet<String>) {
        events.insert(self.event.clone());
        if let Some(filter) = &self.filter {
            filter.collect_events(events);
        }
    }

    fn collect_lookahead(&self, alias: &str, attribute: &str, days: &mut u32) {
        if let Some(filter) = &self.filter {
            filter.collect_lookahead(alias, attribute, days);
        }
    }
}

impl Expr {
    fn children(&self) -> Vec<&Expr> {
        match self {
            Expr::Number(_) | Expr::Bool(_) | Expr::Now | Expr::Attribute { .. } => vec![],
            Expr::Not(expr) | Expr::Negate(expr) => vec![expr],
            Expr::Binary(_, left, right) => vec![left, right],
            Expr::Exists(source) => source.filter.iter().collect(),
            Expr::First { value, source } => {
                let mut children = vec![value.as_ref()];
                children.extend(source.filter.iter());
                children
            }
            Expr::Call(_, args) => args.iter().collect(),
        }
    }

    fn collect_events(&self, events: &mut BTreeSet<String>) {
        match self {
            Expr::Exists(source) => events.insert(source.event.clone()),
            Expr::First { source, .. } => events.insert(source.event.clone()),
            _ => false,
        };
        for child in self.children() {
            child.collect_events(events);
        }
    }

    fn collect_lookahead(&self, alias: &str, attribute: &str, days: &mut u32) {
        if let Expr::Binary(BinaryOp::Add, left, right) = self
            && let Expr::Attribute {
                qualifier,
                attribute: name,
            } = left.as_ref()
            && qualifier == alias
            && name == attribute
            && let Expr::Call(Function::Days, args) = right.as_ref()
            && let [Expr::Number(n)] = args.as_slice()
        {
            *days = (*days).max(n.ceil().max(0.0) as u32);
        }
        for child in self.children() {
            child.collect_lookahead(alias, attribute, days);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Number(f64),
    Symbol(&'static str),
}

const SYMBOLS: [&str; 13] = [
    "<>", "<=", ">=", "(", ")", ",", "=", "<", ">", "+", "-", "*", "/",
];

fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            rest = &rest[c.len_utf8()..];
        } else if c.is_ascii_alphabetic() || c == '_' {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
                .unwrap_or(rest.len());
            tokens.push(Token::Name(rest[..end].to_string()));
            rest = &rest[end..];
        } else if c.is_ascii_digit() {
            let end = rest
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(rest.len());
            let number = rest[..end]
                .parse()
                .map_err(|_| format!("数として読めない: {}", &rest[..end]))?;
            tokens.push(Token::Number(number));
            rest = &rest[end..];
        } else if let Some(symbol) = SYMBOLS.iter().find(|s| rest.starts_with(**s)) {
            tokens.push(Token::Symbol(symbol));
            rest = &rest[symbol.len()..];
        } else {
            return Err(format!("読めない文字: {c:?}"));
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        self.position += 1;
        token
    }

    fn is_keyword(&self, keyword: &str) -> bool {
        matches!(self.peek(), Some(Token::Name(name)) if name == keyword)
    }

    fn is_symbol(&self, symbol: &str) -> bool {
        matches!(self.peek(), Some(Token::Symbol(s)) if *s == symbol)
    }

    fn keyword(&mut self, keyword: &str) -> Result<(), String> {
        match self.next() {
            Some(Token::Name(name)) if name == keyword => Ok(()),
            other => Err(format!("{keyword} がない（{other:?}）")),
        }
    }

    fn symbol(&mut self, symbol: &str) -> Result<(), String> {
        match self.next() {
            Some(Token::Symbol(s)) if s == symbol => Ok(()),
            other => Err(format!("{symbol} がない（{other:?}）")),
        }
    }

    fn name(&mut self) -> Result<String, String> {
        match self.next() {
            Some(Token::Name(name)) if !KEYWORDS.contains(&name.as_str()) => Ok(name),
            other => Err(format!("名前がない（{other:?}）")),
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        match self.next() {
            Some(Token::Number(number)) => Ok(number),
            other => Err(format!("数がない（{other:?}）")),
        }
    }

    /// `<イベント> as <別名> [where <条件>]`
    fn source(&mut self) -> Result<Source, String> {
        let event = self.name()?;
        self.keyword("as")?;
        let alias = self.name()?;
        if alias.contains('.') {
            return Err(format!("別名に . は使えない: {alias}"));
        }
        let filter = if self.is_keyword("where") {
            self.next();
            Some(self.expr()?)
        } else {
            None
        };
        Ok(Source {
            event,
            alias,
            filter,
        })
    }

    fn aggregate(&mut self) -> Result<Aggregate, String> {
        let Some(Token::Name(name)) = self.next() else {
            return Err("集計（ratio・avg・median・variance）がない".to_string());
        };
        self.symbol("(")?;
        let aggregate = match name.as_str() {
            "ratio" => Aggregate::Ratio(self.expr()?),
            "avg" => Aggregate::Avg(self.expr()?),
            "median" => Aggregate::Median(self.expr()?),
            "variance" => {
                let kpi = self.name()?;
                self.keyword("by")?;
                let key = self.expr()?;
                self.keyword("having")?;
                self.keyword("count")?;
                self.symbol(">=")?;
                let min_count = self.number()?;
                if min_count < 0.0 || min_count.fract() != 0.0 {
                    return Err(format!("having count の下限は 0 以上の整数: {min_count}"));
                }
                Aggregate::Variance {
                    kpi,
                    key,
                    min_count: min_count as u32,
                }
            }
            other => return Err(format!("知らない集計: {other}")),
        };
        self.symbol(")")?;
        Ok(aggregate)
    }

    fn expr(&mut self) -> Result<Expr, String> {
        let mut left = self.and()?;
        while self.is_keyword("or") {
            self.next();
            left = Expr::Binary(BinaryOp::Or, Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut left = self.not()?;
        while self.is_keyword("and") {
            self.next();
            left = Expr::Binary(BinaryOp::And, Box::new(left), Box::new(self.not()?));
        }
        Ok(left)
    }

    fn not(&mut self) -> Result<Expr, String> {
        if self.is_keyword("not") {
            self.next();
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let left = self.additive()?;
        let op = match self.peek() {
            Some(Token::Symbol("=")) => BinaryOp::Eq,
            Some(Token::Symbol("<>")) => BinaryOp::Ne,
            Some(Token::Symbol("<")) => BinaryOp::Lt,
            Some(Token::Symbol("<=")) => BinaryOp::Le,
            Some(Token::Symbol(">")) => BinaryOp::Gt,
            Some(Token::Symbol(">=")) => BinaryOp::Ge,
            _ => return Ok(left),
        };
        self.next();
        Ok(Expr::Binary(op, Box::new(left), Box::new(self.additive()?)))
    }

    fn additive(&mut self) -> Result<Expr, String> {
        let mut left = self.multiplicative()?;
        loop {
            let op = if self.is_symbol("+") {
                BinaryOp::Add
            } else if self.is_symbol("-") {
                BinaryOp::Sub
            } else {
                return Ok(left);
            };
            self.next();
            left = Expr::Binary(op, Box::new(left), Box::new(self.multiplicative()?));
        }
    }

    fn multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.unary()?;
        loop {
            let op = if self.is_symbol("*") {
                BinaryOp::Mul
            } else if self.is_symbol("/") {
                BinaryOp::Div
            } else {
                return Ok(left);
            };
            self.next();
            left = Expr::Binary(op, Box::new(left), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.is_symbol("-") {
            self.next();
            return Ok(Expr::Negate(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(Token::Number(number)) => Ok(Expr::Number(number)),
            Some(Token::Symbol("(")) => {
                let expr = self.expr()?;
                self.symbol(")")?;
                Ok(expr)
            }
            Some(Token::Name(name)) => match name.as_str() {
                "true" => Ok(Expr::Bool(true)),
                "false" => Ok(Expr::Bool(false)),
                "now" => Ok(Expr::Now),
                "exists" => {
                    self.symbol("(")?;
                    let source = self.source()?;
                    self.symbol(")")?;
                    Ok(Expr::Exists(Box::new(source)))
                }
                "first" => {
                    self.symbol("(")?;
                    let value = self.expr()?;
                    self.keyword("for")?;
                    let source = self.source()?;
                    self.symbol(")")?;
                    Ok(Expr::First {
                        value: Box::new(value),
                        source: Box::new(source),
                    })
                }
                _ => {
                    if let Some((function, arity)) = Function::from_name(&name) {
                        self.symbol("(")?;
                        let mut args = vec![self.expr()?];
                        while self.is_symbol(",") {
                            self.next();
                            args.push(self.expr()?);
                        }
                        self.symbol(")")?;
                        if args.len() != arity {
                            return Err(format!(
                                "{name} の引数は {arity} 個（{} 個ある）",
                                args.len()
                            ));
                        }
                        return Ok(Expr::Call(function, args));
                    }
                    if KEYWORDS.contains(&name.as_str()) {
                        return Err(format!("ここに {name} は書けない"));
                    }
                    match name.rsplit_once('.') {
                        Some((qualifier, attribute))
                            if !qualifier.is_empty() && !attribute.is_empty() =>
                        {
                            Ok(Expr::Attribute {
                                qualifier: qualifier.to_string(),
                                attribute: attribute.to_string(),
                            })
                        }
                        _ => Err(format!("属性でない名前: {name}")),
                    }
                }
            },
            other => Err(format!("式がない（{other:?}）")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_population_aggregate_and_lookahead() {
        let formula = KpiFormula::parse(
            "from lesson.completed as f where f.is_first_lesson = true
             ratio(exists(lesson.completed as l where l.student_id = f.student_id
                          and l.occurred_at < f.occurred_at + days(60)))",
        )
        .unwrap();
        assert_eq!(formula.population_event(), "lesson.completed");
        assert_eq!(formula.population_alias(), "f");
        assert_eq!(formula.lookahead_days("occurred_at"), 60);
        assert_eq!(formula.lookahead_days("starts_at"), 0);
        assert_eq!(
            formula.events().into_iter().collect::<Vec<_>>(),
            ["lesson.completed"]
        );
    }

    #[test]
    fn reads_variance() {
        let formula = KpiFormula::parse(
            "from lesson.completed as f variance(retention by f.coach_id having count >= 5)",
        )
        .unwrap();
        assert_eq!(formula.referenced_kpi(), Some("retention"));
        assert!(matches!(
            formula.aggregate,
            Aggregate::Variance { min_count: 5, .. }
        ));
    }

    #[test]
    fn rejects_unknown_aggregate_and_trailing_tokens() {
        assert!(KpiFormula::parse("from a.b as a sum(1)").is_err());
        assert!(KpiFormula::parse("from a.b as a avg(1) 2").is_err());
        assert!(KpiFormula::parse("from a.b as a avg(clamp(1, 2))").is_err());
    }
}
