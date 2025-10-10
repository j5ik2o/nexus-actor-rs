# RcShared 対応計画（2025-10-09）

## 背景と目的
- `thumbv6m-none-eabi`（RP2040）では `alloc::sync::Arc` が未提供（CAS 非対応）なため、現状の `actor-core` はコンパイル不可。
- 既存の `shared` 抽象を拡張し、`Rc` バックエンドでも動作可能な共有モデルを導入する必要がある。
- `embedded_rc` フィーチャで `RcShared` を利用する際、`Send + Sync` 制約を段階的に緩和しつつ、std/async 環境では現在のスレッドセーフな設計を維持する。

## 制約
- `Arc` 非対応ターゲットでは `Send + Sync` を満たさないクロージャ／ハンドラが発生し得るため、API の型境界を条件付きに切り替える必要がある。
- 既存の std 向け挙動・テストを regress させないこと。
- 変更範囲が広いため、段階的なマージ（特に `actor-core`）と十分なテストが必須。

## 対応方針（段階的計画）
1. **ArcShared の条件付きフォールバック導入**
   - `nexus-utils-core` に `SharedBound`（Send/Sync 必須かどうかを切り替えるマーカー）を追加。
   - `target_has_atomic = "ptr"` が偽の場合は `ArcShared` を `Rc` ベースに差し替える実装へ変更。

2. **共有クロージャ API の抽象化**
   - `SharedFn` / `SharedFactory` を `SharedBound` ベースに再定義。
   - `MapSystemShared` / `FailureEventHandlerShared` など `actor-core/src/shared.rs` のクロージャ型を `SharedBound` に置換。

3. **メッセージ送受信まわりの型境界整理**
   - `InternalMessageSender` や `MessageSender` が保持するクロージャの `Send + Sync` 制約を `SharedBound` へ変更。
   - `DynMessage`（`Box<dyn Any + Send + Sync>`）の扱いを分岐可能にし、embedded 構成では `Send + Sync` を要求しない別型を検討。

4. **Mailbox / Runtime レイヤ調整**
   - `MailboxFactory` トレイトや `InternalActorRef` などで要求している `Clone + Send + Sync` を条件付きに書き換え。
   - `embedded_rc` でのみ有効化されるテスト／例の境界条件を更新。

5. **ビルド・テストマトリクス整備**
   - CI もしくはローカルの make タスクに `thumbv6m-none-eabi` でのクロスビルドを追加。
   - 既存の std テストがすべて通ること、および `cargo build -p nexus-actor-embedded-rs --no-default-features --features alloc,embedded_rc` が通ることを確認。

6. **リスクと留意点**
   - `Send + Sync` を緩めることで std 環境での安全性が下がらないよう、条件付きコンパイルを厳密に管理する。
   - API 互換性が変化する箇所（公開型の境界変更）が発生する見込み。メジャーバージョン更新のタイミングを検討。
   - embedded/single-thread 専用のドキュメント追加と、例 (`rp2040_basic` 等) の更新が必要。

7. **次のアクション（レビュー後着手想定）**
   - [ ] 上記ステップ 1 の `SharedBound` 導入 + `ArcShared` 条件分岐を実装する PR を準備。
   - [ ] その後、ステップ 2〜4 を段階的に分割し、影響範囲を限定した PR として提出。
   - [ ] 設計レビューで問題なければ、CI 設定やドキュメント更新に取り掛かる。

レビュー後に本計画へ合意が得られ次第、順次実装タスクへ着手する。


## Step3 詳細計画（Pending）

### Scope A: メッセージ送受信境界の整理
- [ ] `InternalMessageSender` / `MessageSender` の `Send + Sync` 要件をプラットフォーム別に切り替える。
    - std: `Send + Sync`
    - embedded_rc: single-thread 前提のローカル送信 (`ArcShared` が `Rc` の場合)`
- [ ] `Ask` 実装（`AtomicWaker` 使用箇所）を `target_has_atomic` で分岐し、非アトミック環境ではローカル waker に落とし込む。
- [ ] `MetadataTable` などグローバル状態が `Spin Mutex` に依存している箇所を `critical-section` ベースへ移行。

### Scope B: Mailbox/Runtime 境界
- [ ] MailboxConcurrency マーカーと ThreadSafe/SingleThread 仕組みを導入する。
- [ ] 各 MailboxFactory が所属モードを宣言し、関連境界を更新する。
- [ ] `MailboxFactory` の associated type にプラットフォーム別境界 (`RuntimeBound`) を導入。
- [ ] `ActorCell` / `PriorityScheduler` / `InternalActorRef` が保持する queue/signal 型のトレイト境界を `RuntimeBound` に更新。
- [ ] `ReceiveTimeoutSchedulerFactory` が要求する `Send + Sync` を条件付きに整理（std→`Send + Sync`, embedded→none）。

### Scope C: DynMessage と Any 境界
- [ ] `DynMessage` を `Box<dyn Any + Send + Sync>` から `Box<dyn Any + RuntimeBound>` 相当へ再設計。
- [ ] タイプダウンキャスト (`downcast`) まわりの安全性確認とテスト追加。

### Scope D: クロスビルド・テスト整備
- [ ] `thumbv6m-none-eabi` ターゲットでのクロスビルド CI step を追加。
- [ ] `embedded_arc` 構成 (Embassy/Arc) のクロスビルド・テスト整備。
- [ ] `embedded_rc` 例 (`rp2040_behaviors_greeter`) の手動手順を README/ドキュメントに追記。
- [ ] std/embedded 双方の smoke テストを `make` タスクへ統合。

### リスク管理
- std 向け互換性を第一優先とし、各 scope を独立した PR に分割。
- `DynMessage` の再設計は Scope C で別ブランチに切り出し、先行して Scope A/B の変更から着手する。
- 各 scope の終了後に `thumbv6m-none-eabi` ビルド確認を実施し regression を防ぐ。
