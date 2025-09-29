# ActorContextExtras 同期化設計ドラフト

## 目的
- ActorContext 周辺のロック競合を軽減し、メッセージ処理のレイテンシを削減する。
- PidSet や MessageHandles を含む周辺型を同期ロック/lock-free 構造へ置き換え、非同期 API 依存を減らす。

## 現状整理
- `ActorContextExtras` は `Arc<RwLock<ActorContextExtrasInner>>` を介してアクセスし、内部フィールドも `RwLock` を用いるためロックの二重化が発生している。
- フィールドの多くが clone で `PidSet` や `MessageHandles` を返却しており、await が多発するホットパスとなっている。
- ReceiveTimeoutTimer は `Arc<RwLock<Pin<Box<Sleep>>>>` を保持しており、リセット操作ごとに await が必要。

## 提案アーキテクチャ
### InnerImmutable / InnerMutable 分割
- `InnerImmutable`: `children`, `watchers`, `extensions`, `context` を格納し、基礎情報を読み取り専用で保持。
- `InnerMutable`: `stash`, `receive_timeout_timer`, `restart_stats` を格納し、変更頻度が高い領域を `Mutex` または `parking_lot::Mutex` で管理。
- `ActorContextExtras` は `Arc<InnerImmutable>` + `Arc<Mutex<InnerMutable>>` を保持し、読み取り操作と更新操作を分離する。

### ロック戦略
- 読み取りパス: 可能な限り `InnerImmutable` から直接参照を返す（必要に応じて `Arc` クローン）。
- 更新パス: `InnerMutable` のみロックし、await を跨がない設計を徹底する。必要な非同期操作はロック外に移動。
- `PidSet` / `MessageHandles` を同期ロックまたは lock-free 化し、`await` を排除する。内部で `DashSet` / `SegQueue` などの選択を比較。

## API 影響
- `get_children` / `get_watchers` / `get_stash` などの戻り値を同期構造に合わせて更新。
- `restart_stats` は `Option<Arc<RestartStatistics>>` を返し、呼び出し側で lazy 初期化を制御する案を検討。
- `init_or_reset_receive_timeout_timer` などのメソッドは非同期境界を見直し、ロックと await を混在させないよう改修。

## マイグレーション案
- `feature = "legacy_context_extras"` を導入し、旧 API を併存させながら段階的に移行。
- PoC ブランチで新 API を試験し、remote/cluster モジュールへの影響を `remote endpoint watcher 調整案` と連携して吸収。
- テストは新旧両方を走らせ、CI で比較検証。

## リスク
- 同期ロック化により single-thread ランタイムでのブロッキングが発生する懸念。
- `PidSet` / `MessageHandles` API の破壊的変更に伴う下位互換性の喪失。
- ReceiveTimeoutTimer の再設計でタイミングが変わる可能性。

## オープン課題
- [ ] `PidSet` と `MessageHandles` の具体的なロック戦略を決定する。
- [ ] lock-free 構造を採用した場合のメモリ使用量・複雑性評価。
- [ ] Remote/cluster への API 影響を追加調査し、移行ステップを文書化。
