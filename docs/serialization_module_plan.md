# シリアライゼーションモジュール実装計画

## ゴール
- remote 系・persistence 系の両モジュールから共通利用できるシリアライゼーション抽象を提供し、transport 依存コードから切り離す。
- `actor-core` が持つ `no_std` ポリシーを維持したまま、共通化されたシリアライザ登録・解決・エラー処理を実現する。
- 既存 wire プロトコル（protoactor-go 互換の serializer ID 等）との整合性を保ちつつ、将来の拡張（独自フォーマット追加など）に備える。

- ドメインメッセージ ↔ 中間表現（型 ID・型名・バイト列・任意メタ情報など）への双方向変換をサポートする。
- 実行時／ビルド時どちらでも型登録が行える仕組みを用意し、グローバルな可変状態を制御可能な形に閉じ込める。
- コアクレートは `alloc` のみで成立させ、`std` 依存の便利機能（ログ、ストレージ、prost 等）は上位層で提供する。
- シリアライザ ID は予約レンジとユーザー定義レンジを区別し、衝突やバージョン差異に対応できるようにする。
- ProtoBuf / JSON（serde-jackson 相当）など複数実装を同時登録・選択可能にし、Akka/Pekko のように用途やメッセージ種別ごとにデフォルト／フォールバックを切り替えられる仕組みを備える。

## クレート構成案
- クレート名: `serialization-core`（ワークスペース新規メンバー）。
- フィーチャ: 既定で `alloc` を有効。必要に応じて `std` を任意追加（例: エラーデバッグ用 `std::error::Error` 実装）。
- 想定モジュール:
  - `id`: `SerializerId` 型、予約済み範囲、`TryFrom` 実装など。
  - `error`: `SerializationError` / `DeserializationError` と補助メソッド。
  - `message`: `SerializedMessage` 構造体（型名・ID・バイト列・ヘッダ情報等）。
  - `serializer`: `Serializer` トレイト（`serialize` / `deserialize` / `type_name`）、`DynMessage` との橋渡しヘルパー。
- `registry`: `SerializerRegistry` トレイトとデフォルト実装（`Arc` + `RwLock` など、`alloc` ベースで実装可能な構造）。
- `selection`: メッセージごとに優先シリアライザを解決するためのルール（デフォルト、型別オーバーライド、フォールバックチェーン）。

## 統合ロードマップ
0. **Phase 0 – Extension 基盤の再整備（前提タスク）**
   - `Extension` トレイトと `ExtensionId` 相当の型安全な登録／取得 API を再設計し、`ActorSystem` から型指定で拡張を取得できるようにする。
   - コア層は `no_std` + `alloc` 前提のため、`ArcShared`／`spin::RwLock` など既存の共有プリミティブを活用し、`std::sync` 依存を持ち込まない。
   - 単体テストを追加し、再入や競合を再現するケースをカバー。

1. **Phase A – クレート初期化**
   - `serialization-core` を追加し、上記モジュールと最小限のユニットテスト（スタブ JSON/バイト列）を実装。
   - `cargo check --no-default-features -p serialization-core` が通ることを確認し、`no_std` 対応を担保。

2. **Phase B – actor-core 連携**
   - `DynMessage` から `SerializedMessage` への橋渡しユーティリティを追加。
   - メッセージ登録ヘルパー（例: `register_message::<T>("fully.qualified.Name")`）を提供し、タイプ情報を保持。
   - 必要に応じて内部テストやドキュメントを更新。

3. **Phase C – remote-* 移行**
   - 既存のシリアライザ実装（`docs/sources/.../remote-std/src/serializer.rs` に記録された旧コード）を新 API に沿って Rust 版へ移植。
   - `RemoteConfig::with_serializer` など設定系 API を `serialization-core` のレジストリへ置き換え、用途別に Proto / JSON 等を切り替えられる設定プロファイルを提供。
   - リモート関連テスト（JSON / Proto roundtrip、EndpointReader など）を更新し、回帰を確認。

4. **Phase D – persistence-* 取り込み**
   - ジャーナル／スナップショット保存処理を共通シリアライザ経由へ変更し、データ種別に応じたシリアライザ選択ロジック（例: スナップショット=ProtoBuf、メタデータ=JSON）を実装。
   - persistence テストスイートでエンコード・デコード失敗時の挙動をカバー。

5. **Phase E – ドキュメント/サンプル整備**
   - 新規ドキュメント `docs/serialization_design.md` を作成し、設計と利用例を記述。
   - remote / persistence のガイドを更新し、新抽象の導入手順を解説。

## テスト戦略
- `serialization-core`: レジストリ登録・取得、ID 変換、エラー伝播のユニットテストを追加。
- 各フェーズ統合後: `cargo test --workspace` で全体回帰。加えて `no_std` チェック用の `cargo check --no-default-features` を CI に組み込み。
- シナリオテスト: リモート通信・永続化それぞれでシリアライザ未登録／失敗ケースを網羅し、期待エラーが返ることを確認。
- マルチ実装のテスト: 同一メッセージに対する複数シリアライザ登録、優先順位／フォールバックが期待通りに働くかを検証。

## 既知の課題 / 検討事項
- 型名や追加メタデータをどのように永続化／共有するか（`lazy_static` 相当と `once_cell` のどちらを採用するか）。
- 登録ポリシー: コンパイル時マクロ (`inventory` 等) と実行時登録の両立をどう設計するか。
- サードパーティ拡張時の ID 衝突防止ルール（予約レンジ、公表用ドキュメント）の策定。
