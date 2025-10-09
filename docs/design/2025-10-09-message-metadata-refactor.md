# MessageMetadata Typed 化計画メモ (2025-10-09)

## 背景
- 現在の `MessageMetadata` は `InternalMessageSender` を直接保持しており、Typed API (`ActorRef<U, R>`) から untyped な sender を露出している。
- `Ask` や `Context::respond` がメタデータを経由して再送信する関係で、`DynMessage` 前提の内部構造に引きずられている。
- 利用者視点では `MessageSender<U>` だけで完結したいが、現状の API では `InternalMessageSender` を扱う必要がある。

## 段階的な移行方針（進捗状況）
1. **Typed sender を優先的に公開する** ✅ *完了*
   - `ActorRef` の `request*` 系 API はすべて `MessageSender<S>` を取る形に統一済み。
   - `Context::request*` や Ask API も typed センダーを利用し、利用者に `InternalMessageSender` を触らせない状態が実現できた。

2. **メタデータ内部に typed/untyped の橋渡し層を用意する** ✅ *完了*
   - `MessageMetadata` を薄いラッパーとして再設計し、内部実装を `InternalMessageMetadata` に隔離。
   - `Context` 自身が typed メタデータを保持し、`ActorContext` には橋渡し用の一時領域を持たせない構造に整理（Drop ガードは不要）。
   - Ask responder も typed センダーで完結するため、レスポンス経路で untyped を扱わずに済む。`ask::dispatch_response` を追加し、`Context::respond` から再利用。
   - `ActorRef::ask_with` / `Context::ask` を追加し、メッセージ側に `replyTo` を受け取るファクトリを渡せるようにした。

3. **最終的に完全な typed 化へ移行する** ⏳ *継続タスク*
   - `ActorContext` や scheduler が保持するメタデータ構造をさらに整理し、typed メタデータのみで回す方向を検討する。
   - `UserMessage<U>` ／ `MessageEnvelope<U>` ／ `DynMessage` を調整し、内部でも `InternalMessageMetadata` への依存を最小化する。
   - `InternalMessageSender` は型消去層に限定し、可能な限り `MessageSender<U>` を直接持つ。

## 残タスク・検討事項
- `ActorContext` 内部のメタデータ格納方式の再整理（完全 typed 化 or typed/untyped ブリッジの最小化）。
- Ask responder の補助 API（例: `MessageMetadata::respond_with` といった糖衣）を用意するかどうか検討。
- `DynMessage` に型付きメタデータを組み込む設計（ecs 的に metadata を別ストレージに持つ案も含めて比較）。
- ジェネリック化によるコンパイル時間／バイナリサイズへの影響評価。

## サイドテーブル方式の叩き台
- 目的: `DynMessage` のサイズを増やさずに typed メタデータをランタイムから参照可能にする。
- 基本方針:
  - `InternalMessageMetadata` を `Arc` で保持し、`DynMessage` 生成時にキー（`u64` など）を払い出す。
  - キーとメタデータの対応は `Scheduler` 側で管理する（例: `slab::Slab<Arc<InternalMessageMetadata>>`）。
  - `PriorityEnvelope` にはキーのみ埋め込み、`ActorCell::dispatch_envelope` でキーからメタデータを取り出し、`Context::with_metadata` に渡す。
  - 処理完了後は必ずキーを解放し、リーク防止のユニットテストを追加する。
- 想定変更箇所:
  - `modules/actor-core/src/api/actor/actor_ref.rs`: `wrap_user_with_metadata` でキーを取得し `PriorityEnvelope` へ埋め込み。
  - `modules/actor-core/src/runtime/mailbox/messages.rs`: `PriorityEnvelope` に `metadata_key: Option<MetadataKey>` を追加。
  - `modules/actor-core/src/runtime/message/mod.rs` 付近に `MetadataTable`（Slab 管理）の新モジュールを追加。
  - `modules/actor-core/src/runtime/scheduler/actor_cell.rs`: `dispatch_envelope` でキー解決とクリーンアップを実施。
  - `modules/actor-core/src/api/messaging/message_envelope.rs`: `UserMessage` はメタデータを `Arc` キーへ移譲し、保持コストを削減。
- 検証タスク:
  - enqueue/dequeue パスでキー管理が O(1) であることをベンチマーク（`criterion` などを利用）。
  - キー未解放の検出と Panic ハンドリング時の挙動確認。

## ベンチ・計測準備案
- 新規 `benches/metadata_table.rs` を追加し、以下を測定:
  1. 現行実装（`UserMessage` にメタデータ内包）での enqueue/dequeue 時間。
  2. サイドテーブル案での同等処理。
- ベンチ対象 API:
  - `ActorRef::tell_with_metadata` → `ActorCell::dispatch_envelope` の流れを模した micro bench。
  - メタデータ無しケース（`MessageMetadata::default()`）と有りケースを分けて比較。
- 結果は設計メモに追記し、閾値（例: 5% 以内）を超える regress があれば DynMessage 拡張案へフォールバック検討。

---
このメモは段階的な実装計画を共有するためのものであり、実作業に着手する際は各ステップごとに PR / 設計レビューを行う。
- `DynMessage` に型付きメタデータを紐付ける方法の検討。候補:
  - `DynMessage` を拡張して `Option<MessageMetadata>` を保持。（型安全になるがサイズ増）
  - メタデータを別レイヤ（例: スレッドローカル or スケジューラ側のサイドテーブル）で管理し、`DynMessage` はキーだけ渡す。
  - メリット／デメリット、パフォーマンス、互換性を比較し、今後の方向性を決める。
- Ask レスポンダー向け糖衣 API の検討。
  - 例: `MessageMetadata::respond_with(actor_ctx, message)` のようなメソッドを提供し、`Context::respond` をより薄く保つ。
  - 併せてユニットテストや例示コードを整備する。
- 利用者向けドキュメント更新。
  - API の変更点（typed dispatcher／メタデータ）を README や examples に反映。
  - サンプルコードで新しいヘルパーの使い方を示す。
- 公開 API では `replyTo` ベースの ask を提供し、現行の `request_*` は internal API として扱う。
