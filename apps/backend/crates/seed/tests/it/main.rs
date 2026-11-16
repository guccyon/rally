//! seed の結合テストを 1 つのテストバイナリにまとめる入口（Issue #1634）。
//! tests/ のファイルごとにバイナリを作るとリンクの時間と target の容量がかさむので、各ファイルをここで `mod` として読む。
//! テストのファイルを足したら、ここにも `mod` を足す（名前の順）。

mod access_token_command;
mod slot_supply;
mod touch_accounts;
