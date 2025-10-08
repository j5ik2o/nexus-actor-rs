# Mailbox Runtime 抽象設計メモ

## 現行構成の把握
- `QueueMailbox<Q, S>` が共通ロジックを保持し、`QueueRw` と `MailboxSignal` の組み合わせで完結する。
  - `try_send` / `recv` / `close` / `is_closed` を一元化済み。
  - `Flag` により `QueueError::Disconnected/Closed` を graceful に処理。
- ランタイム固有層は以下の責務に限定されている。
  1. 適切な `QueueRw` 実装を構築する（`ArcMpsc*` / `RcMpsc*` など）。
  2. ランタイムに応じたシグナル (`Notify` / `Signal` / ローカル waker) を提供する。
  3. `QueueMailboxRecv` を newtype で包み、`Future` を `Pin` で委譲する。

## ランタイム別差分
| 層 | std (`tokio_mailbox.rs`) | embedded (`arc_mailbox.rs`) | local (`local_mailbox.rs`) |
| --- | --- | --- | --- |
| Queue | `ArcMpscUnbounded/BoundedQueue` (容量指定可) | `ArcMpscUnboundedQueue<M, RM>` (`RawMutex` 切替) | `RcMpscUnboundedQueue` |
| Signal | `tokio::sync::Notify` (`Send + Sync`) | `embassy_sync::Signal<RM, ()>` (RawMutex 必須) | `LocalSignal` (`!Send`) |
| Send ハンドル | `QueueMailboxProducer` の newtype (`Send` 可) | 同左 (`Send` は RM 次第) | 同左 (`!Send`) |
| `recv` Future | `TokioMailboxRecvFuture` | `ArcMailboxRecvFuture` | `LocalMailboxRecvFuture` |
| 追加 API | 容量付き `new(capacity)` | `RawMutex` ジェネリクス | なし |

## 抽象化できるポイント
1. **Mailbox 構築パターンの共通化**
   - `QueueMailbox::new(queue, signal)` に対してランタイム層は「Queue と Signal を生成する」だけ。
   - `QueueMailbox` + `QueueMailboxProducer` のペア生成を `actor-core` にヘルパとしてまとめられる。
2. **Recv Future の newtype 重複**
   - 各モジュールで `Future for XxxMailboxRecvFuture` が同一実装。
   - `QueueMailboxRecv` を直接返すか、`RecvFuture` 用の共通ラッパを core 側で用意できる。
3. **Close / Wakeup フロー**
   - `close` 時に `queue.clean_up()` → `signal.notify()` → `Flag::set(true)` は全ランタイムで同じ。
   - シグナルは `notify()/wait()` さえ提供すればよい点を明示的にトレイトに落とし込んでいるため、更なる差分は無し。

## ランタイム差分として残すべき要素
- **シグナル実装**: Wake の呼び出し先（tokio, embassy, local executor）が異なるため、各ランタイムで具体型を持つ必要あり。
- **Queue パラメータ**: `TokioMailbox` のように有界/無界切替や `RawMutex` 選択など、構築時のフラグはランタイム層で扱うのが自然。
- **`Send` / `'static` 制約**: embedded/local では `!Send` な Future も許容したいので、抽象化層で不要な境界を付けないこと。

## インターフェース案
```rust
pub trait MailboxFactory {
  type Signal: MailboxSignal;

  /// ランタイム固有のキュー型。
  type Queue<M>: QueueRw<M> + Clone
  where
    M: Element;

  /// Mailbox を構築し、利用側に producer と本体を返す。
  fn build_mailbox<M>(&self, options: MailboxOptions) -> (QueueMailbox<Self::Queue<M>, Self::Signal>, QueueMailboxProducer<Self::Queue<M>, Self::Signal>)
  where
    M: Element;
}

pub struct MailboxOptions {
  pub capacity: QueueSize,
  pub backpressure: BackpressureMode,
  // embassy 用に RawMutex を選択できるよう追加項目を検討
}
```

- `MailboxFactory` を `actor-core` に配置し、std / embedded / local はそれぞれ `TokioRuntime`, `EmbassyRuntime`, `LocalRuntime` を実装。
- `build_mailbox` を通じて `QueueMailbox` を生成すれば、各ランタイムの `Mailbox` 実装は薄いアダプタで済む。
- `QueueMailboxRecv` を直接返す API も `actor-core` から公開済みなので、利用側では `type RecvFuture<'a> = QueueMailboxRecv<'a, Q, S, M>` をそのまま使える。2025-10-07 現在は `Future<Output = Result<M, QueueError<M>>>` として実装されており、閉鎖・切断時には `Err(QueueError::Disconnected)` を即座に返す。利用側では `Ok` / `Err` の分岐で停止処理を行う。
- 優先度付き Mailbox は **制御キュー（PriorityQueue）＋通常キュー（FIFO）** の二段構えに再編済み。制御メッセージは `priority > DEFAULT_PRIORITY` で判定し、排出時は制御キュー優先。その後 `PriorityScheduler` が余剰メッセージを優先度順に処理する。
- `PriorityEnvelope` に `PriorityChannel::{Control, Regular}` を導入し、API (`control`, `is_control`, `*_control_with_priority`) から明示的に制御メッセージをマーキング可能にした。従来の `priority` 数値はスケジューラのソート指標として残しつつ、キューの振り分けはチャネル属性で判断する。また mailbox の `close()` は制御シグナルを発火させ、待機している `recv` future に `Disconnected` エラーを返す実装となった。
- `ActorContext` は `spawn_child` / `spawn_control_child` を提供し、ハンドラ内から子アクターを生成できる。生成リクエストは `ChildSpawnSpec` として蓄積し、`PriorityScheduler` がディスパッチ後に `ActorCell` へ登録する。これにより子アクターの mailbox/supervisor/handler を共通処理でセットアップできる。
- `SystemMessage` 列挙体と `PriorityEnvelope::from_system` を追加し、protoactor-go の制御メッセージ優先度表を Rust 側でも再現。`PriorityEnvelope::map` でユーザー定義メッセージ型へ容易に変換できる。
- `PriorityActorRef<SystemMessage>` は `try_send_system` を提供し、Supervisor や Guardian から制御メッセージを送信する際にチャネル・優先度が自動で設定される。`PriorityScheduler` の回帰テストで protoactor-go と同様にシステムメッセージが先行処理されることを確認済み。
- `Guardian<M, R, Strat>` を core に実装。protoactor-go の Guardian と同様に、子アクターの制御用 `PriorityActorRef<SystemMessage, R>` を保持し、`GuardianStrategy` の結果に応じて `try_send_system(SystemMessage::Restart|Stop)` を送出する。

## MailboxOptions と優先度 Mailbox の容量解釈
- `MailboxOptions::capacity` は論理的な総容量。優先度 Mailbox では `QueueSize::Limited(total)` をレベル数で均等割り（切り上げ）し per-level capacity として利用。`QueueSize::Limitless` は per-level 0（= dynamic）として扱う。
- Tokio / embedded 双方で同じポリシーを実装。embedded 側は `ArcPriorityQueue::set_dynamic(false)` を容量ありのケースに設定。
- `PriorityScheduler` は Mailbox が FIFO でも優先度順を保証できるよう、`QueueRw::poll()` で排出した `PriorityEnvelope` を一時バッファに集約し、優先度降順でソート後に処理する。既存の TestMailboxFactory でも優先度テストを実施可能。

## 次の検討事項
1. `MailboxOptions` に RawMutex 選択など embedded 固有の情報をどう埋め込むか。
2. `TokioMailbox` の容量指定 (`usize`) を `QueueSize` へ揃えるか要判断。
3. `QueueMailboxRecv` が `Flag` 経由で `Poll::Pending` を返し続ける closed ケースについて、上位層の扱い（停止検知）設計を見直す。

## 段階的リファクタリング計画
1. **ヘルパ導入 (actor-core)**
   - `MailboxFactory` トレイトと `MailboxOptions` を新設。
   - 既存 `QueueMailbox` / `QueueMailboxProducer` を返す `build_mailbox` を追加し、API テストを作成。
2. **std ランタイム移行**
   - `TokioMailbox` を `TokioRuntime` + 汎用 `MailboxHandle` に置換。
   - `tokio_mailbox.rs` では容量設定と `Notify` シグナル構築だけを担当。
   - 既存テストを `TokioRuntime` 版へ更新し、`Send` 境界の維持を確認。
3. **embedded ランタイム移行**
   - `ArcMailbox` / `LocalMailbox` をそれぞれ `EmbassyRuntime` / `LocalRuntime` で再構築。
   - `RawMutex` 選択を `MailboxOptions` で受け取る or `EmbassyRuntime` の型パラメータに残す方式を比較検討。
   - ローカル実行 (`!Send`) シナリオ向けに `QueueMailboxRecv` の型エイリアスだけで済むか確認。
4. **共通 Future ラッパ整理**
   - `QueueMailboxRecv` を直接返すインターフェースを `Mailbox` トレイト利用側に広め、重複 newtype を削減。
   - `Mailbox` トレイトの `RecvFuture` 既定型を `QueueMailboxRecv` ベースに変更できるか調査。
5. **Utils 側の必要拡張**
   - `nexus_utils_*` の queue builder をランタイム問わず構築できるよう、`ArcMpscUnboundedQueue::with_mutex<R>()` 等の API 拡張を検討。
   - `Flag` に `reset()` などが必要か、Mailbox の close 判定要件を洗い直す。
6. **ドキュメント更新**
   - `docs/design` に抽象更新の背景と利用ガイドを追加。
   - `progress.md` にマイルストーン完了ログを追記。

## 今後のタスク切り分け
- 上記 1〜2 を一括リファクタリング対象とし、PR1 で std 側まで完了させる。
- PR2 で embedded/local を移行し、no_std/embassy ビルドを再検証。
- 並行して utils の API 拡張を別 PR で行い、依存サイクルを避ける。

---

## embedded_arc ビルド検証まとめ（2025-10-06 再調査）
- `cargo test -p nexus-actor-embedded-rs --features embedded_arc` 実行時の主なビルドエラー:
  1. `embassy_sync::signal::SignalFuture` が公開されておらず、`ArcSignal<RM>` の `WaitFuture` で解決不可。
  2. `ArcMpscUnboundedQueue<E, RM>` が `Clone` を実装しておらず、`QueueMailbox` の `Q: Clone` 制約を満たせない。
  3. `ArcSignal<RM>` の `#[derive(Clone, Debug)]` が `RM: Clone` を要求（`Signal<M, T>` が `Debug` 非対応）。
  4. embedded テストで利用する `init_arc_critical_section` が `nexus-utils-embedded-rs` では `cfg(test)` 非公開のため参照不可。

- 対応案:
  - **Signal Future**: `MailboxSignal::wait()` が `impl Future` を返す形を前提にし、具象型名の露出を回避する。`ArcSignal` は `Signal::wait()` の戻り値をそのまま associated type に割り当てる（`type WaitFuture<'a> = impl Future<Output = ()> + 'a` の導入も検討）。
  - **Queue Clone**: `ArcShared` が内部で `Arc` を保持するため、`ArcMpscUnboundedQueue`/`ArcMpscBoundedQueue` に手動で `Clone` を実装し、`MpscQueue` のストレージを複製できるようにする。
  - **ArcSignal Debug**: `Debug` 派生を削除 or 手動 `Debug` 実装を用意し、`RawMutex` に余計な trait 境界を課さない。`Clone` 派生は `ArcShared` の `Clone` だけに依存させ、`RM` へ伝播しないよう調整。
  - **Testing Helper**: `init_arc_critical_section` を `nexus-utils-embedded-rs` で `#[cfg(any(test, feature = "test-support"))]` として公開し、`embedded_arc` テストでは該当 feature を有効化する、もしくは `actor-embedded` 側でテストを integration suite に移す。

- 2025-10-06 18:15 更新:
  - `ArcSignal` を手動 `Clone` 実装に変更し、`SignalFuture` 依存を Box 化した `ArcSignalWait` で解消。
  - `ArcMpscUnboundedQueue` / `ArcMpscBoundedQueue` に `Clone` 実装を追加。
  - `cargo test -p nexus-actor-embedded-rs --features embedded_arc` を含むテストが通過する状態を確認済み。

## MailboxFactory 利用レイヤ整理
- 実装済み runtime
  - `TokioMailboxFactory` （std）
  - `LocalMailboxFactory` （Rc ベース）
  - `ArcMailboxFactory` （embedded_arc 用、現状はビルド未整備）
- 既存の呼び出しポイント
  - `modules/actor-std/src/lib.rs:24` などで mailbox を直接生成してから `actor_loop` に渡している。
- 今後の設計方針
  - `Context` / `Supervisor` / `Scheduler` を再設計する際、`MailboxFactory` を初期化時に注入。アクター生成フローで `runtime.build_mailbox()` を呼び出す。
  - テスト用にシンプルな runtime 実装（リングバッファ/モックキュー）を用意し、`MailboxFactory` の trait オブジェクト化を検討。
  - API スケッチ（案）:
    ```rust
    pub struct ActorSystem<R>
    where
      R: MailboxFactory,
    {
      mailbox_rt: R,
      scheduler: Scheduler,
    }

    impl<R> ActorSystem<R>
    where
      R: MailboxFactory,
    {
      pub fn spawn<M, A>(&self, actor: A) -> ActorRef<M>
      where
        M: Element,
        A: Actor<Message = M>,
      {
        let (mailbox, sender) = self.mailbox_rt.build_default_mailbox::<M>();
        self.scheduler.register(actor, mailbox, sender)
      }
    }
    ```
    - `Context`/`Supervisor` は `ActorRef` 経由で `MailboxFactory` に触れる必要がない形を目指す。
    - テスト時は `TestMailboxFactory` を実装し、`VecDeque` ベースのキュー + 手動 wake を提供する想定。

## 優先度付き Mailbox 設計タスク
1. `ArcPriorityQueue` / `RcPriorityQueue` の API を確認し、`QueueRw` と同等の操作性を持つラッパーを提供する。（TokioPriorityMailboxFactory については VecDeque ベースのプロトタイプを実装済み。embedded 向けは未着手）
2. `MailboxOptions` に優先度ポリシーやキュー種別（FIFO / Priority）を受け取るフィールドを追加。
3. `PriorityMailboxFactory`（仮称）を actor-core に導入し、std / embedded でのラッパーを用意。
4. メッセージ `Envelope` に優先度メタデータを持たせ、スケジューラが優先度を参照できる設計を行う。
5. 高優先度メッセージが先に処理されることを検証する unit/integration テストを用意。

## 追記履歴
- 2025-10-06 16:05 メモ初稿
- 2025-10-06 17:45 embedded_arc 課題 & runtime 呼び出し箇所整理、優先度付き Mailbox 設計案追記
- 2025-10-07 10:00 TokioPriorityMailboxFactory プロトタイプ（VecDeque + PriorityEnvelope）実装と容量制御テストの結果を反映
- 2025-10-07 10:26 MailboxOptions 容量ポリシーと PriorityScheduler 実装メモを追加（優先度順ソート／テスト戦略含む）
