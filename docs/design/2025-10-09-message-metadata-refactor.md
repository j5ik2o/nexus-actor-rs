# MessageMetadata Typed 化計画メモ (2025-10-09)

## 背景
- 現在の `MessageMetadata` は `InternalMessageDispatcher` を直接保持しており、Typed API (`ActorRef<U, R>`) から untyped な dispatcher を露出している。
- `Ask` や `Context::respond` がメタデータを経由して再送信する関係で、`DynMessage` 前提の内部構造に引きずられている。
- 利用者視点では `MessageDispatcher<U>` だけで完結したいが、現状の API では `InternalMessageDispatcher` を扱う必要がある。

## 段階的な移行方針（進捗状況）
1. **Typed dispatcher を優先的に公開する** ✅ *完了*
   - `ActorRef` の `request*` 系 API はすべて `MessageDispatcher<S>` を取る形に統一済み。
   - `Context::request*` や Ask API も typed ディスパッチャを利用し、利用者に `InternalMessageDispatcher` を触らせない状態が実現できた。

2. **メタデータ内部に typed/untyped の橋渡し層を用意する** ✅ *完了*
   - `MessageMetadata` を薄いラッパーとして再設計し、内部実装を `InternalMessageMetadata` に隔離。
   - `Context` は `ActorContext` から取り出したメタデータを一度だけ型変換してキャッシュし、ハンドラ終了時は自動でクリアするよう Drop ガードを導入。
   - Ask responder も typed ディスパッチャで完結するため、レスポンス経路で untyped を扱わずに済む。

3. **最終的に完全な typed 化へ移行する** ⏳ *継続タスク*
   - `ActorContext` や scheduler が保持するメタデータ構造をさらに整理し、typed メタデータのみで回す方向を検討する。
   - `UserMessage<U>` ／ `MessageEnvelope<U>` ／ `DynMessage` を調整し、内部でも `InternalMessageMetadata` への依存を最小化する。
   - `InternalMessageDispatcher` は型消去層に限定し、可能な限り `MessageDispatcher<U>` を直接持つ。

## 残タスク・検討事項
- `ActorContext` 内部のメタデータ格納方式の再整理（完全 typed 化 or typed/untyped ブリッジの最小化）。
- Ask responder の補助 API（例: `MessageMetadata::respond_with` といった糖衣）を用意するかどうか検討。
- `DynMessage` に型付きメタデータを組み込む設計（ecs 的に metadata を別ストレージに持つ案も含めて比較）。
- ジェネリック化によるコンパイル時間／バイナリサイズへの影響評価。

---
このメモは段階的な実装計画を共有するためのものであり、実作業に着手する際は各ステップごとに PR / 設計レビューを行う。
