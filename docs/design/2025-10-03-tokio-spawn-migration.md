# tokio::spawn 移行優先度リスト (2025-10-03)

## 区分基準
- **本番コード（P系）**: ランタイムに常駐するタスク。CoreSpawner への移行を最優先。
- **補助ユーティリティ（U系）**: ランタイム補助/同期機構。CoreSpawner 設計検証に活用。
- **テスト（T系）**: ユニット/統合テスト。実装完了後に段階的に移行。

## 優先度一覧

| 優先 | 種別 | ファイル/行 | 用途メモ |
|------|------|--------------|----------|
| P1 | P | `modules/utils-std/src/runtime/sync.rs:170-236` | ✅ CoreSpawner (`TokioCoreSpawner`) へ移行済み。 |
| P2 | P | `modules/remote-std/src/endpoint_manager.rs:260-333` | ✅ Heartbeat/Reconnect タスクを CoreSpawner に刷新。 |
| P3 | P | `modules/cluster-std/src/provider.rs:359-428`, `cluster.rs:238-307` | ✅ Registry Watch/Heartbeat/Remote 起動を CoreSpawner に統一。 |
| P4 | P | `modules/remote-std/src/endpoint_writer.rs:219` | ✅ Stream listener を CoreSpawner に移行済み。 |
| P5 | P | `modules/remote-std/src/endpoint_reader.rs:210-360` | ✅ Receive ストリーム監視と切断タスクを CoreSpawner へ移行済み。 |
| P6 | U | `modules/utils-std/src/concurrent/wait_group.rs:72` | ✅ WaitGroup テストを CoreSpawner ベースへ置換済み。 |
| P7 | U | `modules/utils-std/src/collections/mpsc_*_queue/tests.rs` | ✅ キュー検証タスクを CoreSpawner に移行し join で回収。 |
| P8 | T | `modules/actor-std/src/actor/process/future/tests.rs` | ✅ テスト内 spawn を CoreSpawner + detach へ刷新。 |
| P9 | T | `modules/remote-std/src/tests.rs` | ✅ リモート統合テストの並列処理を CoreSpawner / join に統一。 |
| P10 | T | `modules/cluster-std/src/cluster.rs:261,350` | ✅ Registry/heartbeat 系テストを CoreSpawner 利用へ移行。 |
| P11 | T | `modules/actor-std/src/actor/context/actor_context/tests.rs`, `actor/core/spawn_example/tests.rs`, `actor/dispatch/mailbox/tests.rs` | ✅ 残存テストの tokio::spawn を CoreSpawner 系 util へ置換。 |

## 移行ステップ
1. P1 を CoreSpawner に置換し、`TokioCoreSpawner` の API を確立。
2. P2〜P5 を順次移行し、リモート/クラスタの長時間タスクを CoreScheduler と連動させる。（✅ 完了）
3. P6 でユーティリティ層を対応し、CoreSpawner 導入範囲を拡大。（✅ 完了）
4. テスト (P7〜P11) を CoreSpawner ベースへ移行。（✅ 完了）

## 備考
- 実装・テストの `tokio::spawn` は全て CoreSpawner へ置換済み。残っているのはドキュメント例と `examples/` 以下のデモコードのみ。
- CoreSpawner 移行に合わせて `tokio::time::sleep` などの直接利用も `Timer` 抽象へ統合するか、今後のリファクタ検討課題として保持する。
