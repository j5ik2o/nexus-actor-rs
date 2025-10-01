# core ActorContext / Props 抽象化プラン (2025-10-01)

## 区分基準
- **現状課題**: no_std 化を阻む Context/Props 依存を棚卸し。
- **抽象設計**: core 層へ導入する最小トレイトとデータ構造。
- **アダプタ方針**: actor-std で既存 `ActorContext` / `Props` を橋渡しする方法。
- **移行ステップ**: コード変更と検証順序。

## 現状課題
- `ActorContext` が Tokio runtime / `ActorSystem` / Mailbox へ直接依存し、no_std + alloc 環境で共通実装が利用できない。
- `Props` が MailboxProducer / Middleware など std 固有箇所を内包しており、core 層から再利用できる抽象が存在しない。
- core 側のアクターメッセージ API（`CoreMessageEnvelope` 等）と Context の橋渡しが std 側に散在し、テスト時にモック化しづらい。

## 抽象設計
1. **CoreActorContext トレイト**
   - 必須情報のみに絞る：`self_pid()`、`sender_pid()`、`message()`、`headers()` 等（すべて clone 可能な値を返す）。
   - `Any + Send + Sync` を継承し、std 側アダプタがダウンキャスト可能にする。
   - 拡張用メソッドはデフォルト実装で no-op。
2. **CoreContextBuilder / CoreProps**
   - actor-core に `CoreProps` 構造体を追加し、必要最小限のプロデューサ（`ActorFactory`、`MailboxFactory`）を保持。
   - 生成時に CoreMailbox を要求し、std 側で Tokio mailboxes をラップ。
   - Middleware など std 特有の項目は adapter 側で持ち、core からは `Extensions` 的な Hook だけを受け取る。
3. **エラーハンドリング**
   - `ErrorReasonCore` を利用し、Context 内部で保持する。
4. **API 方針**
   - core トレイトは同期 API（Clone/Owned 値）を返し、std 側で async 化。

## アダプタ方針（actor-std）
- `StdActorContextAdapter` が `ContextHandle` から必要情報を収集し、`CoreActorContext` を実装。
- `StdPropsAdapter` が `Props` 内の MailboxProducer / Middleware を保持しつつ `CoreProps` に変換。
- 既存コードは徐々に adapter 経由へ切り替え、core トレイトの導入効果を検証。
- TODO: `StdActorContextAdapter` が capture すべき追加フィールド（ReceiveTimeout など）を洗い出し、Core トレイトへ反映する。

## 移行ステップ
1. **core 側**
   - `modules/actor-core/src/context/core.rs` を追加し、`CoreActorContext` / `CoreProps` を定義。
   - `lib.rs` から `pub mod context` を公開。
2. **std 側アダプタ**
   - `modules/actor-std/src/actor/context/core_context_adapter.rs` を追加し、`ContextHandle`→`CoreActorContext` 変換を実装。
   - `Props` の最小構成からコア抽象へマッピングするヘルパーを追加。
3. **既存コードの部分置換**
   - Supervisor や core メッセージ処理で `CoreActorContext` を受け取れる API を準備し、順次差し替え。
   - TODO: ContextHandle 利用箇所の台帳を作成し、優先順位に従って Core トレイトへ移行する。
   - TODO: Supervisor/dispatch 層で `StdActorContextSnapshot` を用いる具体 API（メッセージ送信・監視など）を整理するメモを作成。
4. **テスト/ドキュメント**
   - `cargo test --workspace` で回帰確認。
   - 作業ログ (`docs/worknotes/2025-10-01-actor-split-plan.md`) とリリースノート草案を更新。

## リスク
- Props 再設計による広範囲なコンパイルエラー → adapter 経由で段階的に移行。
- Context 情報の async 取得 → アダプタ側で値を Snapshot し、core には同期 API として提供。
