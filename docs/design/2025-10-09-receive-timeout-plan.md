# ReceiveTimeout 提供計画 (2025-10-09)

## 背景と目的
- Akka / protoactor 互換の `ReceiveTimeout` を提供し、一定期間ユーザーメッセージが届かない場合にアクターへシグナルを送る。
- `remote` モジュールでもアイドルチャンネル検知や再接続で必須となるため、MUST 機能として幅優先で整備する。
- `actor-core` は `no_std` を維持する必要がある。タイマー実装は各ランタイム側（まずは `actor-std`）で担当し、コアは抽象のみ定義する。

## 要求事項
1. `Context` から `set_receive_timeout(duration)` / `cancel_receive_timeout()` を呼べるようにする。
2. 受信したユーザーメッセージでタイマーを自動リセットできる構造を用意する。
3. `ReceiveTimeout` シグナルは `SystemMessage` 経由で届ける。`Behavior` から通常のシグナルとして受け取れるようにする。
4. `NotInfluenceReceiveTimeout` 相当のマーカーを維持し、`ReceiveTimeout` 自身や内部メッセージでタイマーがリセットされないようにする。
5. `actor-std` が `tokio` ベースの実装を提供し、`ActorSystem` 初期化時に注入する。
6. 将来的に `actor-embedded` など別ランタイムでも実装できるよう、抽象は汎用的にする。

## 進捗ログ
- 2025-10-09: `utils-core` に DeadlineTimer 抽象（Deadline／Key 管理含む）を追加し、`DeadlineTimerKeyAllocator` のユニーク性テストを整備。
- 2025-10-09: `utils-std` に `TokioDeadlineTimer` を実装し、`tokio_util::time::DelayQueue` をラップするテストを追加。PoC の tokio util 依存を本番コードへ移管可能な状態を確認。
- 2025-10-09: `utils-embedded` に `ManualDeadlineTimer` を追加。`advance` ベースのソフトウェアタイマーで期限を管理し、キャンセル／リセットと単体テストを整備。

## 設計判断と理由
- **抽象と実装の分離**  
  - core は `no_std` 制約下で動作させる必要があるため、タイマー実装を一切持ち込まない。`ReceiveTimeoutScheduler` / `DeadlineTimer` を抽象として定義し、実装はランタイム側に委譲する。  
  - この構成により、`actor-std`（tokio）と `actor-embedded`（ソフトウェアカウンタ）で共通のコアロジックを再利用できる。
- **SystemMessage を経由した通知**  
  - タイムアウト通知は `PriorityEnvelope<SystemMessage>` として送信し、他の制御メッセージと同じ優先度制御を受ける。  
  - 過去の実装で発生した循環参照問題は、メールボックスの producer ハンドルだけを scheduler に渡すことで解決し、`ActorCell` から逆参照が無くなった。
- **DeadlineTimer のキー変換**  
  - `tokio_util::time::DelayQueue` は独自のキーを内部管理するため、`TokioDeadlineTimer` では Key のマッピングテーブルを持ち、外部 API には一貫して `DeadlineTimerKey` を見せる。  
  - これにより core から見たキー管理はランタイム実装に依存せず、他のランタイム実装でも同じキー型を再利用できる。
- **組み込み向けの歩進タイマー**  
  - `ManualDeadlineTimer` はハードウェアタイマーが無い環境でも動作するよう `advance` を明示的に呼び出すデザインにした。  
  - `poll_expired` を Future として実装しておくことで、組み込みランタイム側は `futures` ベースのインターフェースをそのまま活用できる。

## アーキテクチャ案

### 1. コア層（`actor-core`）
- **ReceiveTimeoutScheduler Trait**
  ```rust
  pub trait ReceiveTimeoutScheduler: Send {
    fn set(&mut self, duration: Duration);
    fn cancel(&mut self);
    fn notify_activity(&mut self);
  }
  ```
  - `Duration` は `core::time::Duration` を使用し、`no_std` でも利用可能な最小 API に留める。
  - メッセージ送信用の依存は持たず、抽象は純粋にインターフェースのみ。実際の送信は呼び出し側が行う。

- **Scheduler Factory 抽象**
  ```rust
  pub trait ReceiveTimeoutSchedulerFactory<M, R>: Send + Sync {
    fn create(&self, actor_ref: InternalActorRef<M, R>, map_system: Arc<MapSystemFn<M>>) -> Box<dyn ReceiveTimeoutScheduler>;
  }
  ```
  - ランタイムが複数存在しても同一抽象で差し替えられるよう、`no_std` 側で trait を定義。
  - `InternalActorRef` や `MapSystemFn` は既存のコア型を再利用し、追加の依存を持ち込まない。

- **Context 拡張**
  - `Context` は `Option<&mut dyn ReceiveTimeoutScheduler>` を保持し、API を薄ラッパとして提供。
  - API 例: `set_receive_timeout`, `cancel_receive_timeout`, `notify_receive_timeout_activity`。

- **SystemMessage 拡張**
  - `SystemMessage::ReceiveTimeout` を追加し、優先度は `DEFAULT_PRIORITY + 8` 程度を想定。

- **メッセージ処理フック**
  - `ActorCell` がユーザーメッセージを処理した後に `notify_activity` を呼び出す。
  - Scheduler が存在しない（`None`）場合は何も行わない。

### 2. ランタイム層の注入（`ActorSystemParts`）
- `ActorSystemParts` に `Option<Box<dyn ReceiveTimeoutSchedulerFactory<M, R>>>` を保持し、アクター生成時に Scheduler を生成する。
- `ActorCell::new` で Scheduler を構築し、`Context::new` に参照を渡す。
- 初期リリース段階で `actor-std` と `actor-embedded` の両方がコンパイル可能なよう、エンベデッド向けには no-op 実装（あるいは簡易タイマー）を準備する。

### 3. ランタイム実装例
- **actor-std (Tokio)**
  - `TokioDeadlineTimer`（DelayQueue ラッパー）と非同期タスクで構成する。キー変換とメールボックスへの送信を scheduler 内に閉じ込める。
  - `TokioMailboxFactory` 初期化時に Scheduler Factory を注入し、`ActorSystemParts` へ渡す。

- **actor-embedded (仮実装)**
  - 初期段階では no-op / 簡易バージョンを用意し、ビルドを通す。
  - 将来、RTOS や bare-metal 向けの実装に差し替え可能なよう、抽象は最初から共通化しておく。

### 4. DeadlineTimer インフラ整備（最初に着手）
- このステップは全レイヤーの前提となるため、ReceiveTimeoutScheduler 実装よりも先に行う。
- **utils-core**
  - `DeadlineTimer` 抽象とキー管理 (`DeadlineTimerKey`) を no_std で定義。細かなタイマー実装は持たず、挿入／再アーム／キャンセルのインターフェースのみ提供。
  - `ReceiveTimeoutSchedulerFactory` はこの抽象を利用してスケジューラを構築する。
- **utils-std**
  - `tokio_util::time::DelayQueue` をラップした実装を提供する。再アーム／キャンセルを O(1) で扱い、PoC の DeadlineTimer をこのラッパー経由で利用する形に置き換える。
- **utils-embedded**
  - 初期段階から「動作する」最小実装を用意し、`no_std` ビルド＋簡単な動作テストまで確認する（例: `embedded-hal` タイマーやソフトウェアカウンタを用いた再アーム／キャンセル）。
  - `embassy` などの async ランタイムを活用する場合でも、そのまま差し替えられる構造を確保する。

### 4. `NotInfluenceReceiveTimeout` 運用
- 既存のマーカー（`ReceiveTimeout`, `SystemMessage`, `AutoReceiveMessage`）を core に整備。
- `Context::send_to_self` 等、内部メッセージ送信時にはマーカーを参照してリセットを抑制。

## 幅優先の実装ステップ
1. **抽象追加（コア）**
   - `ReceiveTimeoutScheduler` trait と API を定義。
   - `SystemMessage::ReceiveTimeout` を追加。
   - `ActorCell` / `Context` にスケジューラ参照を配線。
   - テスト: モックスケジューラで API を確認。

2. **std 実装（Tokio 依存）**
   - `actor-std` に `TokioReceiveTimeoutScheduler` を追加。
   - `ActorSystemParts` へ Factory を組み込み、サンプル・テスト（tokio::test）で挙動確認。

3. **ドキュメントと TODO**
   - このメモに進捗を追記。
   - コア API のリファレンス（docs/README 等）へ追記。

4. **後続改善（Backlog）**
   - DeadlineTimer ベースへの置換（ベンチとロック待ち削減）。
   - remote 層でのアイドル検知統合。
   - embedded 向け簡易スケジューラの検討。

## リスクと緩和策
- **no_std 崩壊リスク**: core では抽象だけ定義し、std 依存を持たないことで回避。
- **Tokio タスクリーク**: `cancel` 呼び出し時にタスクを確実に停止させ、drop ガードで二重終了を防ぐ設計にする。
- **オーバーヘッド**: 初期版は単純実装で良しとし、DeadlineTimer 置換は後続タスクとして保持。

## 備考
- `docs/design/2025-10-09-basic-feature-parity.md` の TODO をこの設計に基づき更新予定。
- 実装後は `remote` 側での利用パス（アイドル接続監視など）を確認し、必要に応じてサンプルを追加する。
