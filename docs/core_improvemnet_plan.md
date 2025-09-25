# core 改善計画

## 目的
- `ActorContext` でのネストした非同期ロックを解消し、自己デッドロックを防ぐ。
- メトリクスおよびユーザーコード呼び出し前に必ずロックを解放する構造に改修する。
- `ExtendedPid::ref_process` のロック保持時間を短縮し、`ProcessRegistry` 参照時のデッドロックを除去する。
- `ProcessRegistry::get_process` のリモートハンドラ再入時にもデッドロックしないようロック取得順序を見直す。
- 変更後の挙動を自動テストで検証し、安全性を担保する。

## タスク一覧
1. `ActorContext` の補助ロック (`extras` / `message_or_envelope`) 取得処理を分離し、`MutexGuard` を待機中に保持しないようにする。（完了）
2. `ActorContext` のコールバック実行箇所を洗い出し、ロック解放後にユーザーコード・メトリクス処理を行うよう再配置する。（完了）
3. `ExtendedPid::ref_process` のキャッシュ取得と `ProcessRegistry` 呼び出しを分離し、二段階ロックに変更する。（完了）
4. 危険なパスを対象にしたリグレッションテスト（ユニット/統合）を整備し、`cargo test -p nexus-actor-core-rs` を実行して退行を防ぐ。（完了）
5. `ProcessRegistry::get_process` のリモートハンドラ再入に対するリグレッションテストを追加する。（完了）

## 進捗状況
- [x] 既存コードのロック構造を調査し、問題箇所を特定。
- [x] `ActorContext` のロック分離実装。
- [x] `ExtendedPid::ref_process` の再設計と実装。
- [x] テスト実行および結果記録。
- [x] `ProcessRegistry` のリモートハンドラ再入検証テスト追加。

## リスクと対応策
- **広範囲な変更による副作用**: 影響範囲を単体テストでカバーし、レビュー時に並行性の観点で再確認する。
- **メトリクス拡張との整合性**: 既存 API 互換性を維持しつつ、再入可能性を手動テストで確認する。
- **非同期ロックの取り違え**: `Arc<RwLock<_>>` など補助ロックの取得はヘルパー関数経由に統一し、保守性を高める。

## 次のアクション
1. Typed PID / Typed Context 系 API での `ActorSystem` 参照形態を棚卸しし、弱参照化ルールとドキュメントを整備する。
2. `ContextAdapter` / BaseActor ブリッジにおける同期ブロック (`block_on`) を排除するための非同期化プランを検討し、実装ロードマップをまとめる。
3. `context_borrow` ベンチ結果を定常的に可視化する仕組み（CI アーティファクト出力や閾値チェック）を設計し、`scripts/` にユーティリティを追加する。

### 完了済みメモ (2025-09-25)
- `ActorContext` に `ContextBorrow<'_>` を導入し、`snapshot` API を差し替え。
- `ActorContext`・`ProcessRegistry`・`GuardiansValue`・`DeadLetterProcess`・`RootContext` で `WeakActorSystem` を採用し循環参照を解消。
- `ActorFutureInner`・`Metrics`・`PidActorRef` を弱参照化し、Future/Bridge 経路での循環を除去。
- デコレーター経路の `ContextHandle` を弱参照キャッシュへ切り替え、欠損時は再生成する仕組みを導入。
- `reentrancy` ベンチに `context_borrow` シナリオを追加し、ホットパスの借用コスト測定を可能にした。
- `scripts/check_context_borrow_bench.sh` を追加し、Criterion の平均応答時間を ms 単位で可視化しつつ閾値チェックできるようにした。

### 負荷試験設計メモ（ドラフト）
- **目的**: 再入・多重送信シナリオ下でデッドロックや平均レイテンシ悪化が発生しないことを確認する。
- **対象**: `ActorContext::invoke_user_message` 経由のメッセージ処理、`ProcessRegistry::get_process` を用いたクロスノード送信、メトリクス経路の再入。
- **指標**: 99 パーセンタイルメッセージ処理時間、`ActorFutureProcess` 完了時間、デッドロック検知（タイムアウト 5 秒超過）。
- **シナリオ**:
  - 1万メッセージを並列に `request_future` 経由で発行し、各ハンドラがメトリクス API を再入呼び出し。
  - リモート PID 解決を 50% の確率で失敗させ、デッドレター経路にフォールバックさせる。
- **実装方針**: `cargo make` ターゲットにベンチ用バイナリを追加し、Tokio の `rt-multi-thread` でベンチ実行。完了後にレポート（JSON）を出力して CI で閾値チェック。
- **実装状況**: `cargo bench -p nexus-actor-bench --bench reentrancy` でベンチを実行し、`scripts/check_reentrancy_bench.sh` により平均応答時間がデフォルト閾値（25ms、`THRESHOLD_NS` 上書き可）をチェック可能。CI 連携では `.github/workflows/bench.yml` が `REENTRANCY_THRESHOLD_NS` （リポジトリ変数）を閾値として使用し、`target/criterion/reentrancy/load` をアーティファクトに保存する。
