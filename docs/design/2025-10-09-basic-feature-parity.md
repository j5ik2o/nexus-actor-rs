# 基本機能パリティ TODO（protoactor-go 対比）

protoactor-go の `actor` パッケージで提供される基本機能と比較し、現状の `nexus-actor-core-rs` / `actor-std-rs` に不足している要素を整理する。以下は優先度高の TODO として扱う。

## TODO / Progress

- [x] **Ask 系 API を整備する**（2025-10-08 完了）

  - `Context::request` / `Context::request_future` / `Context::respond` / `Context::forward` を追加済み。`request_future_with_timeout` でタイムアウトを付与した `AskFuture` も提供。
  - `ActorRef` 側に `request(_future)` / `request_with_sender` / `request_future_with_timeout` を公開し、caller 側の ergonomics を改善。
  - `AskFuture` に `AskError::Timeout` / `ResponderDropped` などの終端状態を実装し、軽量なユニットテストを追加。

- [ ] **ReceiveTimeout をユーザー API に統合する**

   - `Context::set_receive_timeout` / `Context::cancel_receive_timeout` を公開し、`NotInfluenceReceiveTimeout` 相当のメッセージフラグを導入する。
   - `ActorContextExtras` / `DelayQueueTimer` の PoC を本番ロジックへ統合し、`ReceiveTimeout` シグナルを `Behavior` DSL に届ける。
   - 参考: protoactor-go `Context.SetReceiveTimeout` / `Context.CancelReceiveTimeout`、`receive_timeout` ミドルウェア実装。

- [ ] **ライフサイクル・ビヘイビア制御を拡張する**

   - `Started` / `Stopping` / `Stopped` / `Restarting` など、protoactor-go がメッセージとして提供するライフサイクルイベントを `Signal` 拡張として DSL に取り込む。
   - `Behavior::become` / `become_stacked` / `unbecome_stacked` に相当する API を追加し、ステートマシン構築の柔軟性を向上させる。
   - 参考: protoactor-go `behavior.go` (`Become`, `BecomeStacked`, `UnbecomeStacked`)、`lifecycle_test.go`。

- [ ] **監視と停止手段の API を強化する**

   - ユーザー向けに `Context::watch` / `Context::unwatch` / `Context::stop` / `Context::poison` を公開し、停止フロー（即時停止・ポイズン）を選択できるようにする。
   - 監視時に Watch/Unwatch の `SystemMessage` が確実に配送される統合テストを追加する。
   - 参考: protoactor-go `Context.Watch` / `Context.Unwatch` / `Context.Stop` / `Context.Poison` と、それに付随する `ProcessRegistry` の扱い。

- [ ] **ガーディアン／スーパーバイザ戦略のバリエーションを追加する**

   - `OneForOneStrategy` / `AllForOneStrategy` / 指数バックオフ戦略を `GuardianStrategy` 実装として提供する。
   - 戦略切り替え API とテスト（プロセス再起動シナリオ）を整備し、`SupervisorDirective` のバリエーションを網羅する。
   - 参考: protoactor-go `strategy_one_for_one.go` / `strategy_all_for_one.go` / `strategy_exponential_backoff.go`。

各項目の進捗を追跡する際は、関連する設計メモ（例: `docs/design/2025-10-07-typed-actor-plan.md`、`docs/worknotes/...`）にも反映すること。
