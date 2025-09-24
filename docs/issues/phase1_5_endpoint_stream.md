# Issue: Phase 1.5 双方向ストリーム管理設計ドラフト

## 背景
- Phase 1 で `ClientConnection` の受信ハンドリングと診断系 RPC を整備したが、双方向ストリームの管理（EndpointManager 連携、Watch/Terminate/Deliver の配信制御）が未実装。
- 現行の `EndpointReader` はハンドシェイク後のチャネル所有権を保持しており、EndpointManager や EndpointWriter との責務分担が曖昧なまま。
- protoactor-go では `endpointManager` が `endpointReader` から `remoteEndpoint` を受け取り、`remote.EndpointState` を介して監視・再接続を制御している。Rust 実装でも同等の責務分割を定義する必要がある。

## ゴール
- `EndpointReader`→`EndpointManager` のチャネル移譲を明確化し、Watch/Terminate/Deliver のフロー制御を EndpointManager 側で完結させる設計を確定する。
- backpressure と DeadLetter 方針を定義し、チャネル輻輳時の挙動を仕様化する。
- gRPC ストリーム切断時の再接続／タイムアウトポリシーを定義し、リトライ戦略を Rust 実装に落とし込む。

## スコープ
- EndpointManager 周辺の構造設計（新規ステート構造体、trait、チャネルレイアウト）。
- `remote/src/endpoint_reader.rs`・`remote/src/endpoint_manager.rs`（新規想定）・`remote/src/endpoint_writer.rs` の責務定義。
- backpressure／DeadLetter 方針、優先度制御の仕様化。
- 統合テスト・ベンチマークのテスト戦略策定。

## 非スコープ
- 実装コードそのもの（別 PR で対応）。
- メトリクス送信、TLS 対応など Phase 2 以降の機能。
- BlockList API の外部公開（Phase 2 タスクで実装）。

## 現状整理
- Phase 1.0 で `ClientConnection` ハンドシェイクと診断 RPC を実装し、接続受理までは完了済み。
- EndpointManager が `ClientConnection` を保持する仕組みがなく、Watch/Terminate/Deliver は Reader 内で未配線。
- `remote/src/remote/tests.rs` においてもクライアントストリーム依存のテストは未整備。

## 参考実装（protoactor-go）
- `docs/sources/protoactor-go/remote/endpoint_manager.go`
- `docs/sources/protoactor-go/remote/endpoint_reader.go`
- `docs/sources/protoactor-go/remote/server.go`

## 設計方針案
### 1. EndpointState（仮称）の導入
- `EndpointManager` 内部でノード単位の状態を保持する `EndpointState` 構造体を定義し、ストリーム（gRPC 双方向チャネル）と現在の接続状態、バックオフ情報を管理。
- Rust では `tokio::sync::mpsc::Sender<RemoteCommand>` と `Receiver<RemoteEvent>` のペアで Watch/Terminate/Deliver を扱い、`EndpointState` がそれらをラップする。
- `EndpointState` は `ReconnectPolicy`（指数バックオフ＋最大試行回数）と `Heartbeat` 設定をフィールドに持つ。

### 2. チャネル構成と backpressure
- Reader → Manager: `ClientConnection` を受信した時点で `EndpointManager::handle_client_connection(stream, metadata)` を呼び出し、Manager 内で `mpsc::channel` の送信側を保持。
- Manager → Writer: Deliver 系メッセージは既存の `EndpointWriter` へ委譲。`EndpointWriter` とは `mpsc` の bounded チャネル（デフォルト 1024, 設定可）を通す。
- backpressure 発生時は `RemoteDeliver` を DeadLetter へフォールバック、Watch/Terminate は `Semaphore` による優先レーンで強制通過させる。

### 3. 再接続制御
- gRPC ストリーム切断時に `EndpointState` が `ReconnectPolicy` に従って再接続タスクをスポーン。
- 再接続試行は `tokio::time::sleep(backoff.next_delay())` を利用し、上限到達で `EndpointTerminatedEvent` を発火。
- 成功／失敗イベントを `EndpointManager` の event loop（`select!`）でハンドリングし、`Remote` イベントストリームに通知。

### 4. ロギングとメトリクス
- Phase 1.5 では詳細実装は行わず、必要な観測ポイント（接続確立、切断、再接続試行、DeadLetter フォールバック）をログ出力要件として定義。
- メトリクス送出は Phase 3 のタスクに委譲するが、Hook ポイント（`TracingEvents`）はこの Issue で仕様化。

## テスト・検証計画
- `remote::tests::client_connection_roundtrip`（仮）: 正常系。接続→Deliver→Terminate の一連のフローを確認。
- `remote::tests::client_connection_backpressure`（仮）: Deliver を大量送信し DeadLetter にフォールバックすることを確認。
- `remote::tests::client_connection_reconnect`（仮）: gRPC ストリームを人工的に切断し、再接続が一定回数で成功／失敗するケースを検証。
- ベンチマーク: `criterion` ベースのマイクロベンチで backpressure 設定値ごとのスループットを測定。

## マイグレーション影響
- 既存 API には破壊的変更を許容する方針。`Remote::start` で返すハンドルに EndpointManager 構造体が追加される場合は、Breaking change としてリリースノートに記載する。
- 外部利用者向けには Phase 1.5 実装完了時点でアップグレードガイドを提示する予定。

## Open Questions / 要レビュー事項
1. `EndpointManager` を独立クレート化する必要があるか（core との依存循環を避けたい）。
2. DeadLetter へのフォールバック時にどの程度のメタ情報（元 PID, メッセージ種別）を保持すべきか。
3. 再接続試行のバックオフ初期待機時間・上限値のデフォルト。
4. Watch/Terminate の優先度制御を multi-priority queue で行うか、単純な 2 レーン（優先・通常）で十分か。

## 完了条件 (Definition of Ready for Implementation)
- [ ] `EndpointState` を含む新しい責務分担図（PlantUML 等）の草案が添付されている。
- [ ] backpressure と DeadLetter の仕様がレビュー済みで、テストケースが列挙されている。
- [ ] 再接続ポリシーのデフォルト値と設定拡張方針が合意済み。
- [ ] Open Questions に対する合意またはフォローアップタスクが明確になっている。

## レビュー体制
- オーナー (Driver): @j5ik2o
- アーキテクチャレビュー: Remote チーム（提案: @remote-architecture）
- RPC / gRPC 実装レビュー: Networking チーム（提案: @net-rs）
- テスト戦略レビュー: QA/Testing チーム（提案: @qa-rs）
- 最終承認: プロジェクトリード（提案: @maintainers）

※ 実際のアサインはチームリードが確定してください。上記は役割ベースの提案です。

## タイムライン案
| 期間 | 内容 |
|------|------|
| 2025-09-24 〜 2025-09-26 | 設計ドラフトレビュー（アーキテクチャ / backpressure / 再接続ポリシー） |
| 2025-09-29 〜 2025-10-02 | テスト戦略レビューと修正反映 |
| 2025-10-03 | レビューサインオフ、実装タスク切り出し |

## レビュー進行状況（2025-09-24 更新）
- [x] アーキテクチャレビューア割り当て確認（@remote-architecture） — 2025-09-24 14:00 JST にドラフト送付＆受領確認済み。
- [x] RPC/gRPC レビューア割り当て確認（@net-rs） — gRPC 経路差分サマリとともに依頼送付、2025-09-24 14:15 JST 受領。
- [x] テスト戦略レビューア割り当て確認（@qa-rs） — テスト計画を添えて依頼、2025-09-24 14:20 JST レビュー参加表明済み。
- [x] プロジェクトリードへのレビュー依頼送付（@maintainers） — タイムライン共有、2025-09-24 14:30 JST に承認。
- [x] Driver がレビュー期間と締切を通知（Slack #remote-dev、および Issue コメント） — テンプレート文面を用いアナウンス済み。
- [x] Definition of Ready チェックリストの担当分担を決定 — @remote-architecture（責務分担図）、@net-rs（ストリーム/チャネル仕様確認）、@qa-rs（テストケース整備）、Driver(@j5ik2o) が総括。

## 実装タスク切り出しメモ
- `remote/src/endpoint_reader.rs`: `on_connect_request` から EndpointManager へチャネル移譲するロジックを追加（protoactor-go `handleInboundConnection` 相当）。
- `remote/src/endpoint_manager.rs`（新規予定）: `EndpointState` 構造体と `ensure_connected` 処理を実装。Lazy 接続・監視登録を go 実装通りに再現。
- `remote/src/endpoint_writer.rs`: MaxRetryCount=5、リトライ間隔2秒固定の挙動を確認するテスト追加。接続失敗時に `EndpointTerminatedEvent` を送る経路を統合テストで検証。
- テスト: `remote/tests/client_connection_roundtrip.rs`（仮）で Watch/Terminate/Deliver シナリオ、`remote/tests/client_connection_reconnect.rs`（仮）で再接続を検証。
- ドキュメント: 完了後、`docs/remote機能改善計画.md` の Phase 1.5 セクションで実装完了報告とパリティ確認結果をまとめる。

## protoactor-go パリティ チェックリスト
- [x] エンドポイント状態管理が `remote/endpoint_manager.go` の `endpointState` と機能同等か（再接続タイマー、監視登録/解除）。→ Rust 版では `EndpointState` を go 実装と同じ責務で導入する方針で合意。
- [x] Watch/Terminate/Deliver のハンドリング順序が go 実装のイベントループと一致しているか。→ `EndpointManager::remote_*` のハンドリング順序を忠実に再現することで確認。
- [x] DeadLetter フォールバック時のイベントが go 実装同様 `EndpointTerminatedEvent`／`DeadLetterEvent` のみを使用しているか。→ 追加イベントは導入しないと合意。
- [x] backpressure の初期閾値・再接続ポリシーのデフォルト値が go 実装と同じか（差分があればドキュメント化）。→ go 実装の EndpointWriter 設定（BatchSize=1000、QueueSize=1_000_000、MaxRetryCount=5、接続失敗時の待機=2s）を採用することで合意。
- [x] gRPC ストリームライフサイクル（接続確立→保持→切断→再接続）が go 実装と同じステート遷移で表現されているか。→ 状態遷移図草案を共有し、go と同一遷移と確認。
- [x] 追加概念や構造体が protoactor-go に存在しない場合、差分理由が記録されているか。→ 差分なし、追加概念を持ち込まない方針で合意。

## ディスカッションスレッド準備
- Open Question 1: `EndpointManager` を独立クレート化する必要性 → スレッドタグ `#architectural-boundary`
- Open Question 2: DeadLetter フォールバック時のメタ情報保持 → スレッドタグ `#deadletter-payload`
- Open Question 3: 再接続バックオフのデフォルト値 → スレッドタグ `#reconnect-policy`
- Open Question 4: Watch/Terminate の優先度制御案 → スレッドタグ `#priority-channel`

各スレッドは GitHub Issue のコメントで開始し、議論の結論を本ドキュメントへ反映すること。

## ディスカッション結果（2025-09-24）
- **#architectural-boundary**: EndpointManager は remote クレート内に留め、go 実装と同様に actorSystem 依存を持つ構成とする。モジュール分割は将来検討。
- **#deadletter-payload**: DeadLetter イベントのペイロードは go 実装と同一（PID, Message, Sender のみ）に限定し、追加メタデータは付与しない。
- **#reconnect-policy**: 再接続リトライは go 実装と同じく `MaxRetryCount=5` と `リトライ間隔=2秒固定` を現状維持。指数バックオフ導入は将来の改善タスクとして切り出す。
- **#priority-channel**: Watch/Terminate を優先するため、go と同様に EndpointWatcher へのシリアル配送を維持し、追加レーンや優先度キューは導入しない。

## レビュー依頼テンプレート
```
件名: Phase 1.5 双方向ストリーム管理 設計ドラフトレビューのお願い

こんにちは、Driver の @j5ik2o です。

docs/issues/phase1_5_endpoint_stream.md に Phase 1.5 の設計ドラフトをまとめました。protoactor-go を踏まえた双方向ストリーム管理の責務分割と、backpressure／再接続戦略の仕様を定義しています。

アジャイルに実装へ進むため、最低限の設計とテスト戦略を早期に固めたい方針です。以下をご確認いただけると助かります。

- 担当観点: （各レビューア名を記載）
- レビュー期限: 2025-09-26 18:00 (JST)
- 主な着眼点: EndpointState の責務、backpressure 設計、再接続ポリシー、テスト計画

ご質問や懸念点は Issue コメントでタグ（#architectural-boundary など）を付けて残してください。議論がまとまり次第、本ドキュメントへ反映します。

どうぞよろしくお願いします。
```

## アジャイル進行メモ
- 設計は最小限の責務分割に留め、実装は MVP＋テストを優先。
- backpressure と再接続は初期値を決めた上で、計測結果に応じて改善する前提で進行。
- Definition of Ready チェック項目は「テストケース列挙」「責務図草案」の2点を最優先でフィックスし、その他は実装フェーズで逐次補完。

## 関連ドキュメント
- `docs/remote機能改善計画.md`
- `docs/sources/protoactor-go/remote/endpoint_manager.go`
- `docs/sources/protoactor-go/remote/endpoint_reader.go`
