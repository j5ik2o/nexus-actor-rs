# dispatch_all 段階的非推奨ガイド (2025-10-07)

## 背景
`PriorityScheduler::dispatch_all` は初期設計の同期 API として残っているが、Tokio / Embassy を含む各
ランタイムから自然に扱える `dispatch_next` / `run_until` / `run_forever` が整備されたため、今後は
非推奨とし段階的に置き換える。

## 現状
- `dispatch_all` 呼び出し時に `tracing::warn!` を一度だけ発行し、移行を促す。
- 同期版の実装は `drain_ready_cycle` を共有するため、即時の挙動変更は発生しない。
- `run_until` / `run_forever` / `blocking_dispatch_loop` / `blocking_dispatch_forever` を用いれば、従来の
  同期ループと同等の機能を提供可能。

## 推奨移行ステップ
1. **アプリケーションコード**: `dispatch_all` を呼び出している箇所を `run_until` もしくは `dispatch_next`
   ループに置換する。Tokio などの async ランタイムでは `run_forever` をタスクとして起動する。
2. **テストコード**: 同期テストの場合は `futures::executor::block_on(scheduler.run_until(...))` を使用し、
   明示的にループ回数を制御する。
3. **ドキュメント**: ガイドや README に `dispatch_all` がレガシーであることを明記し、推奨パターンを
   `run_until` 系 API へ差し替える。
4. **将来対応**: 次期リリースで `#[deprecated]` 属性を付与し、さらにその次で削除する計画を立てる。

## TODO
- [ ] README / example コードの置換（`dispatch_all` -> `run_until`）。
- [ ] `TypedRootContext` のサンプルを async 版に刷新する。
- [ ] `dispatch_all` が呼ばれる CI テストの棚卸し（テスト専用利用かどうかの確認）。
