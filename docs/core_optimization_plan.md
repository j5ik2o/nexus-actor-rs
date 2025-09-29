# core 最適化メモ (2025-09-29)

## ゴール
- DefaultMailbox・Context 関連の最適化現況を簡潔に記録する。

## 現況
- DefaultMailbox は同期キュー化済み。QueueLatencyTracker/metrics も同期アクセスに移行。
- ContextHandle 系は同期 snapshot 化が完了しており、主要ホットパスでのロック待ちが解消されている。
- `bench.yml` / `bench-weekly.yml` で再入・コンテキストベンチを継続監視中。

## TODO
- Virtual Actor 経路が追加された際に、DefaultMailbox と Context のホットパス測定を再実施。

