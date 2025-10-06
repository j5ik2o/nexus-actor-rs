# ランタイム/クラスタ移行設計 (2025-09-30 時点)

## 区分基準
- **アーキテクチャ層**: ランタイム基盤 / メッセージング抽象 / 分散・クラスタ / 観測・管理の4層で漏れなく重複なく整理する。
- **移行フェーズ**: フェーズ1〜4で PoC → 既存機能移植 → 分散機能再構成 → 旧実装凍結の順に定義し、前提・成果物・ゲート条件を明示する。
- **運用モード**: no_std コア / std 拡張 / エンタープライズサポートの3区分で提供形態を切り分ける。

## 対象範囲
- 現行 `core`/`remote`/`cluster` クレートを段階的に再構成し、新しい no_std 対応ランタイムと std 拡張層を共存させるための設計。
- クラスタ・リモート通信における Transport プラグイン化と、クラスタサービス (`Membership`・`IdentityDirectory`) の責務分離。
- 観測性（メトリクス/ログ/トレース）と運用サポート導線の整理。

## 非対象
- GUI / CLI ツール群の詳細設計。
- 特定プロトコル（例: MQTT, Modbus）の Transport 実装詳細。
- ライセンス条項や価格プランの最終決定。

## アーキテクチャ全体像
1. **ランタイム基盤**: no_std を前提とした `ActorSystemCore`、タスクスケジューラ、ライフサイクル制御、同期コンテキスト API。
2. **メッセージング抽象**: `Envelope`, `Interaction`, ミドルウェアチェーン、固定長メッセージ ID と動的ディスパッチの両立構造。
3. **分散・クラスタ**: Transport 抽象 (`Transport`, `TransportEndpoint`)、`MembershipService`, `IdentityDirectory`, `PlacementStrategy` のプラガブル化。
4. **観測・管理**: `ObservationHub` によるメトリクス/トレース/ログ連携、`ManagementPort` での制御チャンネル統合。

## ランタイム基盤 (MECE: スケジューラ / コンテキスト / ライフサイクル / メールボックス)
- **スケジューラ**: `TaskSlot` と `WorkStealingQueue` を trait 化 (`SchedulerProvider`)、no_std では RTOS タスク、std では Tokio runtime をアダプタで提供。
- **コンテキスト**: `SyncBorrow<'a>` と `AsyncHandle` を分離し、await を伴わないホットパスを保証。`ReenterAfter` はコールバックベースで実装。
- **ライフサイクル**: `LifecyclePolicy` と `FailureStrategy` を DSL で宣言。Supervisor は `SupervisorBuilder` で組立、no_std では静的配列構成。
- **メールボックス**: `MailboxKind`（低遅延/バックプレッシャ/順序保証）の列挙と、固定長バッファを使う `SyncRing` を標準実装。std では `MpmcQueue` 拡張を feature で提供。

## メッセージング抽象 (MECE: Envelope / Interaction / Middleware)
- **Envelope**: `MessageHeader`（kind, correlation_id, payload_hint）と `Payload`（`&[u8]` / `&dyn AnyMessage`）の2層構造。no_std では固定長 ID + 登録テーブル。
- **Interaction**: `fire_and_forget`, `ask`, `expect` を `Interaction` トレイトで統一。no_std 版はブロッキング待機、std 版は async/await ラッパー。
- **Middleware**: `ReceiveFilter`, `SendFilter`, `ErrorHook` を分離し、no_std ではコンパイル時長の静的配列を利用。std 版は Vec で動的追加。

## 分散・クラスタ設計 (MECE: Transport / Cluster Services / 配置戦略)
- **Transport**: `Transport` トレイトで送受信・ストリーム管理を抽象化。実装例: `transport-udp`, `transport-quic`, `transport-serial`。no_std では RTOS ドライバ連携を想定。
- **Cluster Services**: `MembershipService`（参加・離脱・心拍）と `IdentityDirectory`（Virtual Actor 解決）を分離し、それぞれ pluggable backend（InMemory / gRPC / Kubernetes / SharedMemory / RTOS Bus）。
- **配置戦略**: `PlacementStrategy` で Consistent Hash, Rendezvous, Static Partition を切り替え。no_std 環境ではメモリ上限に応じた固定テーブルを選択。

## 観測・管理 (MECE: 観測 / 管理 / サポート)
- **観測**: `ObservationHub` がイベントをファンアウト。no_std ではコールバック経由でホストへ転送、std では OpenTelemetry Exporter・Prometheus メトリクスを feature で接続。
- **管理**: `ManagementPort` トレイトで制御チャネル抽象化（UART/CAN/REST/gRPC）。`Command`/`Response` を protobuf/CBOR のどちらでも扱えるよう serializer 抽象を整備。
- **サポート導線**: エディション別（Core, Std, Enterprise）に設定テンプレート・SLA フック・診断収集 API を提供。

## フェーズ計画
| フェーズ | ゴール | 主な成果物 | ゲート条件 |
|---------|-------|-------------|------------|
| Phase 1: no_std ランタイム PoC | `nexus-actor-core` の新規サブクレート作成、`cargo test --no-default-features --features alloc` パス | ActorSystemCore/SyncContext/SyncMailbox の最小実装、既存ユニットテストの一部を移植 | no_std ビルドが CI で常時成功、PoC ベンチが現行性能を満たす |
| Phase 2: std 互換シムと既存機能移植 | `nexus-actor-std` に Tokio アダプタを実装し、既存 core/remote の API を段階移植 | 旧 core の主要テストが新ランタイム上で成功、互換ラッパー `compat::*` 提供 | 旧コード上の回帰テストを並行実行できる CI マトリクス完成 |
| Phase 3: 分散・クラスタ再構成 | Transport/Cluster Services/PlacementStrategy を分離、既存 remote/cluster 機能をプラグイン化 | gRPC Transport と InMemory Membership を std 拡張として提供、組み込み向け Transport PoC | 分散統合テストが新構成で成功、旧 remote/cluster を段階的に deprecated |
| Phase 4: 旧実装凍結・サポート体制整備 | 新 API をデフォルト化し旧コードを LTS ブランチへ隔離、商用サポート導線を確立 | LTS ブランチ運用方針、移行ガイド、Enterprise SLA テンプレート | 利用者が新 API へ移行可能と判断、社内サポート体制が合意済み |

## no_std / std 運用モデル
- **no_std コア**: `#![no_std]` + `alloc` オプション。Allocator/RealtimePrimitive トレイトでメモリ・同期をホスト依存に注入。診断イベントはコールバック経由。
- **std 拡張**: `tokio`, `tracing`, `prost`, `tonic` 等を feature で有効化。高機能 Transport・管理 API・観測連携をこの層に集約。
- **エンタープライズ**: LTS ブランチ、サポート API（ダンプ収集・構成検証ツール）、Transport プラグイン認証プロセスを提供。契約に応じて独自 Transport やセキュリティ拡張を追加。

## 拡張ポイント
- `SchedulerProvider`, `MailboxFactory`, `TransportFactory`, `MembershipBackend`, `ObservationSink`, `ManagementPort` をトレイト化し、feature gate で実装を選択。
- Serializer 抽象 (`Serializer`, `Deserializer`) を導入し、Protobuf/CBOR/FlatBuffer を選択可能に。no_std ではメモリフットプリントに応じた serializer を選べる。

## テスト・検証計画
- CI マトリクス: `cargo test --workspace` (std) / `cargo test --no-default-features --features alloc` (no_std) / `cargo miri test` (UB 検証) / `cargo bench` (性能確認)。
- 組み込み向け HIL テスト: Transport 模擬デバイスと RTOS タスクでのクラスタ参加 E2E。
- 回帰テスト: 旧 API と新 API の挙動差をゴールデンテストで比較し、互換レイヤー廃止判断の資料とする。

## ビジネス・サポート方針
- エディション区分: Core (FOSS no_std), Standard (std + オープン Transport), Enterprise (サポート契約, 専用 Transport, SLA 工具)。
- サポート提供物: 移行ガイド、デバッグ情報収集 CLI、設定検証スクリプト、主要 KPI レポートのテンプレート。
- コミュニティ運営: OSS 版は GitHub Discussions + Slack、Enterprise は専用チャンネル + インシデント対応窓口。

## TODO / 次アクション
1. `nexus-actor-core` サブクレート雛形と `ActorSystemCore` スケルトンを作成し、no_std ビルドを CI に追加する。
2. 既存ユニットテストからランタイム基礎テストを抽出し、新ランタイム上で動作するよう移植する。
3. Transport 抽象のトレイト定義案を作成し、gRPC/QUIC/Serial の実装スケジュールを策定する。
4. サポート体制に必要な成果物（移行ガイド, LTS ブランチ運用規約, 診断 CLI）の要件定義をまとめる。
