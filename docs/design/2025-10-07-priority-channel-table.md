# Priority Channel Mapping

> 参考: protoactor-go `mailbox/system_message.go` の SystemMessage 定義と旧 nexus-actor-rs の実装をベースに整理。

| メッセージ種別 | 想定チャネル | 備考 |
| --- | --- | --- |
| `SystemMessage::Watch` / `Unwatch` | Control | 監視更新は停止・障害通知より優先して処理したい |
| `SystemMessage::Stop` | Control | 停止要求。終了処理を遅延させない |
| `SystemMessage::Failure` | Control | Supervisor 再起動決定に直結するため優先処理 |
| `SystemMessage::Escalate` | Control | 上位 Guardian / System 側へ障害通知をエスカレーション |
| `SystemMessage::Restart` | Control | 再起動指示。protoactor-go の `Restarting` を参考 |
| `SystemMessage::Suspend` / `Resume` | Control | Mailbox の状態切替。優先度高 |
| Actor ユーザーメッセージ | Regular | `PriorityEnvelope::new` のデフォルト |
| 優先ユーザーメッセージ (例: 優先メールボックス) | Regular (priority 値で順序制御) | `PriorityEnvelope::new` + custom priority |

## 優先度に関する指針

- 制御メッセージは `PriorityEnvelope::control` を介して生成し、チャネルを `Control` に設定する。
- 制御チャネル内では protoactor-go の優先度（0〜100）に倣い、`DEFAULT_PRIORITY + Δ` を割り当てることで FIFO を維持しつつ緊急度を表現できる。
- 通常チャネルは FIFO 処理。必要に応じて `PriorityEnvelope::new(..., priority)` で優先度を調整する。

## 今後の TODO

- [x] SystemMessage 列挙型の導入と `PriorityEnvelope` ヘルパーの連携。`modules/actor-core/src/mailbox.rs` に `SystemMessage` と `PriorityEnvelope::from_system` を追加済み。（2025-10-07）
- [x] 優先度値の標準テーブル化（Stop = +10 等）。`SystemMessage::priority()` で protoactor-go ベースの優先度テーブルを提供。（2025-10-07）
- [x] Supervisor 経由で送信する内部メッセージの一貫テスト。`scheduler::tests::scheduler_prioritizes_system_messages` などで制御メッセージ優先処理を検証済み。（2025-10-07）
