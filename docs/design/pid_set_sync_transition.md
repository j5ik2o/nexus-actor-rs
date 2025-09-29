# PidSet 同期化移行計画

## 目的
- `PidSet` の非同期ロック依存を排除し、ActorContext および remote `EndpointWatcher` での await 減少を図る。
- ベンチマークでロック待ち時間を計測し、同期化の効果を定量化する。

## 現行仕様
- `PidSet` は `Arc<RwLock<Vec<Pid>>>` と `Arc<RwLock<HashMap<String, Pid>>>` を保持し、`add`/`remove` などすべての操作が `async`。
- `PidSet::new()` も `async` で初期化し、remote 側では `await` 前提で利用。
- テスト (`core/src/actor/core/pid_set/tests.rs`) も tokio runtime に依存。

## 変更方針
- 内部構造を同期ロック (`parking_lot::RwLock` など) へ変更し、`async` を廃止。
- `Vec` + `HashMap` の構成は維持しつつ、`SmallVec` や `IndexSet` などの代替を検討。
- `PidSet` API は同期メソッド (`fn add(&mut self, pid: Pid)`) へ変更し、必要に応じて `Arc`/`Mutex` で囲む責務を利用側へ移動。

## API 変更一覧
| メソッド | 現行シグネチャ | 変更後シグネチャ | 備考 |
| --- | --- | --- | --- |
| `new` | `pub async fn new() -> Self` | `pub fn new() -> Self` | 非同期初期化を廃止。
| `add` | `pub async fn add(&mut self, v: Pid)` | `pub fn add(&mut self, v: Pid)` | 内部ロックで同期化。
| `remove` | `pub async fn remove(&mut self, v: &Pid) -> bool` | `pub fn remove(&mut self, v: &Pid) -> bool` | 同期戻り値継続。
| `contains` | `pub async fn contains(&self, v: &Pid) -> bool` | `pub fn contains(&self, v: &Pid) -> bool` | 読み取りロックに変換。
| `len` | `pub async fn len(&self) -> usize` | `pub fn len(&self) -> usize` |  | 
| `to_vec` | `pub async fn to_vec(&self) -> Vec<Pid>` | `pub fn to_vec(&self) -> Vec<Pid>` | クローンは維持。

## テスト移行計画
- 既存テストから `#[tokio::test]` を削除し、標準テストに書き換え。
- 非同期メソッドを想定したテストケースは同期化後の動作に合わせて調整。
- remote `EndpointWatcher` テストも同期 API に対応させる。

## スケジュール案
1. PoC ブランチで core 側のみ同期化してベンチ実行。
2. remote 側の API を同期に合わせて調整。
3. cluster モジュールに依存がないか最終確認し、本体ブランチへ統合。

## リスクと対応策
- ブロッキング化により、非同期タスクの待機が発生する可能性 → ベンチと tokio-console で監視。
- API 変更による外部依存破壊 → `legacy` feature を用意し、段階的移行。
- Lock-free 構造への移行コスト → まず同期ロックで効果検証後に検討。

## TODO
- [ ] PoC 実装でベンチ結果を収集し、docs/benchmarks/core_actor_context_lock.md に反映。
- [x] remote EndpointWatcher への影響範囲を洗い出し、調整スケジュールを確定。(2025-09-26: 同期 API 適用)
- [ ] `legacy_context_extras` feature の要否を評価する。
