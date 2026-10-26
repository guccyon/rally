//! 合成シード v0（T18）。Insight セルの KPI 計算と一周シナリオの E2E を試すための合成データを、application の
//! ユースケースを通して Postgres に書く（不変条件を迂回しない。行動イベントも同じく発火する）。
//!
//! 計画（[`SeedPlan`]。乱数の種だけから決まる純粋な生成）と、その実行（[`Seeder`]）を分ける。書いた結果の
//! 件数と分布は [`SeedReport`] が DB から読む。依存してよいのは domain・application・infra
//! （`crates/architecture/tests/dependency_direction.rs`）。

mod coach_plan;
mod retention;
mod seed_args;
mod seed_error;
mod seed_plan;
mod seed_report;
mod seeder;
mod slot_plan;
mod step;
mod student_plan;

pub use coach_plan::CoachPlan;
pub use retention::Retention;
pub use seed_args::SeedArgs;
pub use seed_error::SeedError;
pub use seed_plan::SeedPlan;
pub use seed_report::SeedReport;
pub use seeder::Seeder;
pub use slot_plan::SlotPlan;
pub use step::Step;
pub use student_plan::StudentPlan;
