//! KPI の式の正本（specs/analytics/kpi.yaml）をファイルから読む。

use std::path::PathBuf;

use application::{Unavailable, analytics::KpiSpecSource};

/// [`KpiSpecSource`] の実装。照会のたびに `path` のファイル（specs/analytics/kpi.yaml。式はこの 1 か所にだけ置き、
/// バックエンドに写さない）を読む。読めなければ [`Unavailable`]。
///
/// 置き場所は `KPI_SPEC_PATH` で渡す（出荷用のイメージは apps/backend だけで作るので、deploy/compose.yaml がリポジトリの
/// kpi.yaml を読み取り専用でつなぐ）。なければリポジトリの中の specs/analytics/kpi.yaml（手元とテスト）。
#[derive(Debug, Clone)]
pub struct FileKpiSpec {
    path: PathBuf,
}

impl FileKpiSpec {
    pub const PATH_VAR: &'static str = "KPI_SPEC_PATH";
    /// リポジトリの中の kpi.yaml（このクレートから見た場所）。
    pub const DEFAULT_PATH: &'static str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../specs/analytics/kpi.yaml"
    );

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// プロセスの環境変数から選ぶ。
    pub fn from_env() -> Self {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// `var`（環境変数の名前から値）の [`Self::PATH_VAR`]。ないか空なら [`Self::DEFAULT_PATH`]。
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        Self::new(
            var(Self::PATH_VAR)
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| Self::DEFAULT_PATH.to_string()),
        )
    }
}

impl KpiSpecSource for FileKpiSpec {
    async fn kpi_yaml(&self) -> Result<String, Unavailable> {
        std::fs::read_to_string(&self.path)
            .map_err(|e| Unavailable::new(format!("{}: {e}", self.path.display())))
    }
}
