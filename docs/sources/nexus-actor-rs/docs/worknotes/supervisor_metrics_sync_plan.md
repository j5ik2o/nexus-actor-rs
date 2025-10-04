# Supervisor / Metrics 経路同期化検討メモ

## 区分基準
- **目的/現状**: 課題の整理。
- **再設計方針**: 検討中の案。
- **リスク**: 想定される懸念点。


最終更新: 2025-09-25

## 目的

- `SupervisorHandle` と `ActorContext` を中心とした監視経路で利用されるメトリクス更新処理を棚卸しし、非同期依存を整理する。
- `metrics_foreach` を再設計し、同期パスと共存可能な共通 API を用意することで、ホットパスの待ち時間とロック競合を削減する。
- protoactor-go の実装パターン（同期実行 + 即時スナップショット）を参考に、Rust 版として適切なイディオムを提示する。

## 現状整理

### 呼び出し箇所一覧

| ファイル | 関数 | 主な役割 | 非同期依存 | 備考 |
| --- | --- | --- | --- | --- |
| `modules/actor-core/src/actor/context/actor_context.rs:300` | `incarnate_actor` | Spawn 時のカウンタ加算 | `metrics_foreach` → `extension_arc.lock().await` | コンテキスト生成直後。 |
| `modules/actor-core/src/actor/context/actor_context.rs:543` | `handle_restart` | 再起動カウンタ加算 | 同上 | 状態遷移中で await が多重に並ぶ。 |
| `modules/actor-core/src/actor/context/actor_context.rs:993` | `StopperPart::stop` | Stop 発生カウンタ | 同上 | 子プロセス停止処理と並列。 |
| `modules/actor-core/src/actor/context/actor_context.rs:1126` | `invoke_user_message` | メッセージ受信時間の記録 | 同上 | メッセージ処理ホットパス。 |
| `modules/actor-core/src/actor/context/actor_context.rs:1160` | `escalate_failure` | Failure カウント更新 | 同上 | Suspend → 親通知の中間で await。 |
| `modules/actor-core/src/actor/context/actor_context.rs:1230` | `Supervisor::escalate_failure` 実装 | Failure カウント + ラベル生成 | 同上 | `Metrics::common_labels` も async。 |
| `modules/actor-core/src/actor/process/dead_letter_process.rs:144` | `Process::send_user_message` | Dead letter カウント | 同上 | Dead letter 監視。 |
| `modules/actor-core/src/actor/process/future.rs:71/194` | Future 完了時 | Future 成功/失敗の統計 | 同上 | `ActorFutureProcess` でも利用。 |
| `modules/actor-core/src/actor/process/actor_future.rs:148` | Future 生成/完了 | 同上 | |

- すべての呼び出しが `metrics_foreach` → `Metrics::foreach` と連鎖し、`Arc<Mutex<Extension>>` を `lock().await` してから async クロージャを実行している。
- 監視経路 (`handle_child_failure`, supervisor 戦略) において、`ActorContext` は非同期ロック中に `SupervisorHandle` を生成し直し、さらにメトリクスでも await するため、`tokio::sync::RwLock` 上に待ちが発生する。
- protoactor-go ではメトリクスアクセスは同期 API であり、Extension から取得した参照を即時に利用するためロック待ちが発生しにくい。

### `metrics_foreach` の問題点

1. **二重の await**: 呼び出し側も async クロージャを受け取り、その中で非同期処理を行うため、`await` の入れ子が生じている。ホットパスでのコンテキストスイッチが多い。
2. **ロック競合**: Extension 取得 (`extension_arc.lock().await`) とメトリクスアクセス (`Arc<Mutex<_>>` など) が連鎖し、Supervisor 層・メールボックス層での同時アクセス時にロック順序が揃わない可能性がある。
3. **共有データのコピー**: `Metrics::common_labels` が都度非同期取得を行い、`ActorSystem` への再入を伴う。反復的な `Arc` clone と `String` 生成が発生。

## 再設計方針

### 方針 1: `metrics_foreach` の同期化

- `Metrics::foreach` を同期 API (`fn foreach(&self, F)`) に変更し、メトリクス操作は軽量な `MetricsSink` に委譲する。
- Extension 取得時に `MetricsSnapshot`（`Arc<ActorMetrics>` + `CommonLabels` + `MetricsConfig`）を構築し、呼び出し側へ貸し出す。
- Snapshot を `once_cell::unsync::OnceCell` 風に `ActorContext` / `SupervisorHandle` に保持し、ホットパスでの繰り返し計算を避ける。

### 方針 2: 同期・非同期の共存レイヤー

- `MetricsAccess` トレイトを定義し、以下 2 種類の API を提供：
  - `fn with_metrics(&self, f: impl FnOnce(&MetricsSink))`（同期）
  - `async fn with_metrics_async(&self, f: impl Future<Output = ()>)`（互換用）
- 既存コードは徐々に同期 API へ移行し、非同期版はレガシーサポートとして残す。
- Supervisor 戦略や `DeadLetterProcess` は即時同期 API へ移行することで、メッセージ処理と同じスレッド内で完結させる。

### 方針 3: `MetricsSink` の抽象化

- `MetricsSink`（仮称）は以下の責務を持つ構造体：
  ```rust
  pub struct MetricsSink {
    actor_metrics: Arc<ActorMetrics>,
    common_labels: Arc<[KeyValue]>,
  }
  impl MetricsSink {
    pub fn inc_actor_spawn(&self) { ... }
    pub fn record_message_duration(&self, seconds: f64, msg_type: Option<&'static str>) { ... }
    // etc.
  }
  ```
- ラベル生成や `Arc` の clone を `MetricsSink` 内に閉じ込め、呼び出し元は同期メソッドを呼ぶだけにする。
- Supervisor 専用のメソッド（`inc_failure_with_labels` など）は `MetricsSink::for_supervisor()` のように別インターフェースを提供。

### 方針 4: Extension キャッシュの導入

- `ActorSystem` 側で `Metrics` Extension を取得したら `Arc<MetricsRuntime>` としてキャッシュし、`ContextBorrow` / `GuardianProcess` / `SupervisorHandle` へ配布する。
- キャッシュは `ArcSwapOption` で保持し、Extension アップデート時にもホットスワップ可能。
- これにより、`metrics_foreach` 内で毎回 Extension を引き当てるための `await` を排除する。

## protoactor-go との比較ポイント

- Go 版は Extension を同期的に取得後、`metric.WithAttributes` で即時メトリクス更新を行う。ラベル生成も `CommonLabels` が同期関数。
- Rust 版では `ActorSystem::get_config()` も同期メソッド化済みで、設定アクセスから await が排除された。残る非同期部分は Context/Extension 初期化フローでの借用整合のみ。
- Go 版の `ctx.props.receiverMiddlewareChain` などは同期呼び出しで、Rust 版は `async fn run(...)`。メトリクスだけ同期化しても、コンテキスト生成が async な限り完全な同期パスは成立しない → 「同期フェーズで snapshot を取得し後続では非同期無し」のハイブリッドアプローチが現実的。

## 提案アーキテクチャ

1. **MetricsRuntime の導入**
   - `MetricsRuntime { actor_metrics: Arc<ActorMetrics>, labels_cache: DashMap<MetricLabelScope, Arc<[KeyValue]>> }` を追加。
   - Extension 登録時に `Arc<MetricsRuntime>` を生成し、`ActorSystem` が `ArcSwapOption` で保持。
   - `ActorContext` / `SupervisorHandle` / `ProcessHandle` に `Option<Arc<MetricsRuntime>>` を注入。

2. **SyncMetricsAccess トレイト**
   - 同期化完了後は不要となったため削除。各コンポーネントは固有の `metrics_sink()` や専用ヘルパー（例: `record_supervisor_metrics`）を通じて同期メトリクスを更新する。

3. **API 置き換え**
   - `metrics_foreach(|am, m| async move { ... }).await;` を
     ```rust
     record_supervisor_metrics(&actor_system, &supervisor, "one_for_one", decision, &child_pid, vec![]);
     ```
     のようにヘルパー関数へ集約。`ActorContext` や Process 系は `metrics_sink_or_init()` など固有メソッドで同期アクセスする。

4. **Supervisor 共存**
   - `SupervisorHandle::inject_snapshot` と同様に、`MetricsSink` も `SupervisorHandle` 内のセルへキャッシュ。再起動後も捕捉済みメトリクスを再利用。
   - Supervisor 戦略側には `SupervisorMetricsCtx` を渡し、同期 API で子プロセス数・失敗回数を更新できるようにする。

## 段階的移行ステップ

1. **ステップ 0: 型導入**
   - `MetricsSink` と `MetricsRuntime` の最小実装を作成し、`ActorContext` に `Option<Arc<MetricsRuntime>>` を保持させる。

2. **ステップ 1: metrics_foreach のインターフェース変更**
   - 既存 API を `deprecated` 扱いにし、新 API (`ActorSystem::metrics_foreach`) を追加。
   - 呼び出し元を順次移行。`ActorContext::incarnate_actor` などホットパスは `metrics_sink_or_init()` で同期アクセスへ移行済み。

3. **ステップ 2: Supervisor 経路の移行**
   - `ActorContext::handle_child_failure` / `handle_root_failure` で `SupervisorMetricsCtx` を生成し、戦略実装へ注入。
   - 監督戦略テストを更新し同期 API を使用。`strategy_one_for_one` / `strategy_all_for_one` / `strategy_restarting` は `ActorSystem::metrics_foreach` を直接呼び出し、`SupervisorHandle::metrics_sink` 依存を段階的に解消済み (2025-09-25)。

4. **ステップ 3: Config / Extension アクセスの同期化**
   - `ActorSystemConfig` を `Arc` 共有できる形に変更し、`get_config()` 同期アクセスでの差し替えを完了。（実績済み）
   - Extension 取得をキャッシュし、await を最小化。

5. **ステップ 4: レガシー async API の削除**
   - すべての呼び出しが `metrics_sink` に移行したら、`metrics_foreach` と `Metrics::foreach` を削除。

## 期待される効果

- メッセージ受信ホットパスでの await 削減により、スループット向上とレイテンシ揺らぎの低減が期待できる。
- Supervisor 戦略内のメトリクス更新が同期化され、`SupervisorHandle` のロック待ち時間が短縮される。
- メトリクス無効時の分岐が高速化し、条件判定のみで早期 return 可能。

## リスクと対応

- **同期 API 導入による共有状態の可視化**: `MetricsSink` が `Arc<ActorMetrics>` を内部に保持するため、誤って `Send` で別スレッドに渡すと想定外アクセスが起きる。→ `MetricsSink` を `#[derive(Clone)]` + `!Send`（`PhantomData<*const ()>`）にして、スレッド越え移動を抑止。
- **キャッシュ整合性**: Extension 差し替え時に古い `MetricsRuntime` が残存する。→ `ArcSwap` によりホットスワップし、古い `Arc` は参照が消え次第破棄。
- **段階移行の複雑化**: 一括リファクタはリスクが高い。→ 各ステップで明示的にテスト（`cargo test -p nexus-actor-core-rs actor::context::actor_context::tests::` 等）を実行するガイドラインを整備。

## まとめ

- `metrics_foreach` を中心とする非同期 API を同期化し、`MetricsSink` へ責務を委譲することで、Supervisor/metrics 経路のホットパスを軽量化する。
- protoactor-go の同期的なメトリクス更新パターンを Rust 向けに翻案し、Extension キャッシュ + Snapshot を活用してラベル生成とロックを最小化する。
- 段階的移行を前提に、新旧 API を共存させながら最終的には同期 API へ統合する。
- `ActorHandle` に型名キャッシュを導入し、Supervisor 側がメトリクス ラベルを生成する際にも同期的に型名へアクセスできるようにした。
