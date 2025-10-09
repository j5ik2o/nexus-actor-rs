# MessageMetadata Typed 化計画メモ (2025-10-09)

## 背景
- 現在の `MessageMetadata` は `InternalMessageDispatcher` を直接保持しており、Typed API (`ActorRef<U, R>`) から untyped な dispatcher を露出している。
- `Ask` や `Context::respond` がメタデータを経由して再送信する関係で、`DynMessage` 前提の内部構造に引きずられている。
- 利用者視点では `MessageDispatcher<U>` だけで完結したいが、現状の API では `InternalMessageDispatcher` を扱う必要がある。

## 段階的な移行方針
1. **Typed dispatcher を優先的に公開する**
   - `ActorRef::request_with_dispatcher` を `request_with_untyped_dispatcher` にリネームし `pub(crate)` 相当に落とす。
   - 代わりに `request_with_typed_dispatcher<S>(&self, message: U, sender: MessageDispatcher<S>)` を追加し、内部で `sender.into_internal()` を呼ぶ実装にする。
   - Context や Ask 系の公開 API でも同様に typed ラッパーを提供し、利用者には untyped を触らせない。

2. **メタデータ内部に typed/untyped の橋渡し層を用意する**
   - `MessageMetadata` を `InternalMessageMetadata` にリネームし、内部実装を隔離。
   - 新しい `MessageMetadata<U>` ラッパーを導入し、`InternalMessageMetadata` との変換 (`from_typed`, `into_internal`) を提供する。
   - Ask responder などランタイム内部では必要に応じて untyped のまま扱い、段階的に typed 化可能な箇所から移行する。

3. **最終的に完全な typed 化へ移行する**
   - `UserMessage<U>` / `MessageEnvelope<U>` / `DynMessage` などを調整し、`MessageMetadata<U>` を直接保持できるよう再設計。
   - `Context::respond` や Ask 経路で `MessageDispatcher<U>` を直接取り出せるようにする。
   - `InternalMessageDispatcher` は必要最小限（型消去層）にのみ留める。

## 残タスク・検討事項
- `InternalMessageDispatcher` と `MessageDispatcher<U>` の両立期間における API 互換性ポリシー。
- Ask responder の typed 化（`MessageEnvelope<Resp>` を返す道筋の明確化）。
- ジェネリック化によるコンパイル時間・バイナリサイズへの影響評価。
- `DynMessage` 自体を型付きメタデータと併存できるよう拡張するか、別レイヤーでマッピングするかの判断。

---
このメモは段階的な実装計画を共有するためのものであり、実作業に着手する際は各ステップごとに PR / 設計レビューを行う。
