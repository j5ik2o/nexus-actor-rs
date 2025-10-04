# Issue: Phase 2 ConfigOption 拡張と BlockList 公開 API

## 区分基準
- **背景/ゴール**: 取り組みの位置付け。
- **設計案**: 実装方針。
- **テスト/ドキュメント**: フォローアップ計画。


## 背景
- Phase 1.5 で EndpointManager/Writer の backpressure と再接続基盤を整備したが、運用時に必要な管理機能（BlockList API、構成の細かな調整）が未整備。
- LAN 内で運用するクラスタを想定しており、TLS は当面不要。代わりに BlockList によるアクセス制御を充実させたい。
- protoactor-go では ConfigOption で統一的に設定を差し替える設計になっており、Rust 版でも同じ体験を提供する必要がある。

## ゴール
- ConfigOption/Config に BlockList 初期化や追加構成項目（再接続・backpressure 関連、今後の拡張余地）を統一的に定義する。
- BlockList を外部コードから操作できる API（block/unblock/list）を `Remote` に公開し、テストで保証する。
- ドキュメントとサンプルを更新し、運用手順を明文化する。

## スコープ
- ConfigOption の拡張（BlockList 初期値設定、再接続/Backpressure パラメータの列挙、将来用の拡張ポイント）。
- `Remote` への BlockList 操作用 API 追加 (`block_system`, `unblock_system`, `list_blocked_systems`) と非同期ハンドリング。
- 結合テスト（BlockList 経由で接続拒否が機能するか）。
- ドキュメント・サンプル更新（設定例、BlockList 操作例）。

## 非スコープ
- TLS ハンドシェイク／証明書管理（Phase 2 では扱わない）。
- Phase 3 以降で予定している EndpointWriter の再接続改修やメトリクス導入。

## 現状整理
- `BlockList` は内部 API のみ提供しており、`Remote` 経由で操作できない。
- ConfigOption には EndpointWriter/Manager のパラメータが追加済みだが、BlockList に関する設定項目が不足。
- テストでは BlockList の拒否動作が十分にカバーされていない。

## 参考実装
- protoactor-go `remote/blocklist.go` と ConfigOption 実装。
- Phase 1.5 で改修した `ConfigOption::with_endpoint_*` と同様のパターン。

## 設計案
### ConfigOption 拡張
- `ConfigOption::with_blocked_systems(Vec<String>)` を追加し、`Config` 側で初期 BlockList を設定できるようにする。
- 再接続／backpressure のデフォルト値を記録するアクセサ（`get_endpoint_reconnect_*`, `get_backpressure_*`）をまとめてドキュメント化。

### BlockList 公開 API
- `Remote::block_system(system_id: &str)` / `unblock_system(system_id: &str)` / `list_blocked_systems() -> Vec<String>` を追加。
- 内部的には `BlockList` の非同期メソッドを await してラップする。

## テスト計画
- ユニットテスト: `BlockList` の操作 API が正しく内部状態を更新すること。
- 結合テスト: `Remote` を起動し、BlockList に登録された system_id の接続を拒否できるか検証。
- ConfigOption テスト: 初期 BlockList 設定が `Config` に反映されるか確認。

## ドキュメント更新
- `docs/` に BlockList 操作手順と設定例を追記。
- `remote/examples/` に BlockList を利用した簡単なサンプルを追加し、`cargo run --example` で検証。

## タイムライン（目安）
| タスク | 期間目安 |
|--------|----------|
| 設計ドラフト作成 | 1日 |
| 実装 (ConfigOption + BlockList API) | 2日 |
| テスト追加・整備 | 1日 |
| ドキュメント・サンプル更新 | 1日 |

ご質問や追加要望があればコメントでお願いします。
