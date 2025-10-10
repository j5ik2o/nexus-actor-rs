# Mailbox Runtime Boundary Design (2025-10-10)

## 背景
- `MailboxFactory` の `Queue`／`Signal` は現状 `Clone` のみ要求し、std ランタイムでは実質的に `Send + Sync` を仮定している。
- `embedded_rc`（`Rc` ベース）では `Send + Sync` を満たさず、今回 Scope A の変更で `InternalMessageSender` などは `target_has_atomic` をキーに要求を切り替えた。
- しかし `MailboxFactory` 自体の境界を同様に緩和しようとすると、std 環境のテスト実装（`TestMailboxFactory`）が `Rc` を利用しているため、ホストビルドで大量の `Send + Sync` 不足が発生する。
- `embedded_arc`（Embassy + Arc 構成）は引き続き `Send + Sync` が必要であり、両者を共存させる仕組みが必要。

## 課題整理
1. **ターゲット判定だけでは不十分**: `target_has_atomic = "ptr"` はホストビルドでも真になるため、テスト用 `Rc` 実装が排除される。
2. **ファクトリ毎に要件が異なる**: std/Tokio 系のファクトリは `Send + Sync` が必須、一方 embedded/Local 系は不要。
3. **関連箇所が広範囲**: `ActorCell` / `PriorityScheduler` / `ReceiveTimeout` / `ActorRef` など、多数の API が`MailboxFactory` の associated type に依存。

## 提案
### 1. MailboxConcurrency マーカー導入
- 新 trait `MailboxConcurrency` を導入し、`ThreadSafe` / `SingleThread` の 2 種類を提供。
- `MailboxFactory` に associated type `Concurrency: MailboxConcurrency` を追加。各ファクトリが自分のモードを宣言する。

```
pub trait MailboxConcurrency {
  type QueueBound<M>: ?Sized;
  type SignalBound: ?Sized;
}

pub struct ThreadSafe;
pub struct SingleThread;
```

- `ThreadSafe` では `QueueBound<M> = dyn QueueRw<M> + Clone + Send + Sync`、`SingleThread` では `QueueRw<M> + Clone` を alias。
- `MailboxFactory` の型境界は `Self::Concurrency::QueueBound<M>` を利用することで、ファクトリごとに必要な制約を切り替えられる。

### 2. 実装方針
1. `MailboxConcurrency` trait と 2 つの具象型を `actor-core/src/runtime/mailbox/traits.rs` に追加。
2. 既存のファクトリを分類:
   - `TokioMailboxFactory`, `TokioPriorityMailboxFactory`, `ArcMailboxFactory` など → `ThreadSafe`
   - `LocalMailboxFactory`, テスト用の `TestMailboxFactory` → `SingleThread`
3. `ActorCell`, `InternalActorRef`, `ActorRef`, `ReceiveTimeoutFactoryShared` などの `Send + Sync` 境界を `MailboxFactory::Concurrency` 経由で参照するよう更新。
4. `RuntimeBound` / `SharedBound` との整合性: `ThreadSafe` の場合に限り `RuntimeBound` を `Send + Sync` に解決するよう調整。
5. `embedded_arc` は `ThreadSafe` を選択することで従来通りの動作を維持。

### 3. テスト戦略
- std 環境のユニットテストは `SingleThread` を宣言しているため引き続き `Rc` を使用可能。
- `embedded_rc` / `embedded_arc` のクロスビルドを Scope D で追加し、`Cargo` タスクまたは CI に組み込む。

## 既知の課題
- `MailboxFactory` に associated type を追加するため、クレート間の API 変更となる。上位クレート (`actor-std`, `actor-embedded`) も同時更新が必要。
- `MapSystemShared` など他の `Send + Sync` ベースの型は引き続き `SharedBound`／`RuntimeBound` を利用する。
- ドキュメントとマイグレーションガイド（breaking change）の整備が必要。

## 次のステップ
1. `MailboxConcurrency` 実装と既存ファクトリの適用（Scope B-1）。
2. `ThreadSafe` コードパスで `Send + Sync` を維持しつつ、`SingleThread` で `RcShared` が動作することを確認。
3. `embedded_arc` / `embedded_rc` 両構成のクロスビルドテストを整備（Scope D）。
4. ドキュメント更新とサンプル修正。
