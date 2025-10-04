# Embassy Scheduler 利用ガイド (2025-10-04)

## 区分基準
- **準備手順**: Embassy executor 初期化から `CoreRuntime` 作成までの流れ。
- **設定ポイント**: スロット数やキャンセル制御に関する注意事項。
- **確認項目**: 動作確認とトラブルシュート。

## 準備手順
1. **タスクスロットの確保**
   ```rust
   use nexus_actor_embedded_rs::spawn::EmbassyTaskSlot;

   static SLOTS: [EmbassyTaskSlot; 4] = [EmbassyTaskSlot::new(); 4];
   ```
2. **Embassy executor と spawner の取得**
   ```rust
   let executor = embassy_executor::Executor::new();
   let spawner = executor.start(|spawner| {
       // `spawner` は SendSpawner に変換可能
   });
   ```
3. **CoreSpawner の生成**
   ```rust
   use nexus_actor_embedded_rs::spawn::EmbassyCoreSpawner;

   let core_spawner = Arc::new(EmbassyCoreSpawner::new(spawner, &SLOTS));
   ```
4. **EmbeddedRuntimeBuilder へ注入**
   ```rust
   use nexus_actor_embedded_rs::embedded::EmbeddedRuntimeBuilder;
   use nexus_actor_embedded_rs::core::runtime::CoreRuntime;

   let runtime: CoreRuntime = EmbeddedRuntimeBuilder::new()
       .with_spawner(core_spawner)
       .build_runtime();
   ```

## 設定ポイント
- `EmbassyCoreSpawner` のスロット数 `N` は同時に走らせたいタスク数に合わせて調整。
- `EmbassyScheduler::drain` を呼ぶことで、全スケジュール済みタスクをキャンセル可能。
- `schedule_repeated` の `interval` が `Duration::ZERO` の場合は単発実行となる。

## 確認項目
- **キャンセル確認**: `CoreScheduledHandle::cancel()` でループ Future が停止するか確認。
- **ノイズ排除**: スロット不足時は `CoreSpawnError::CapacityExhausted` が返るため、スロット拡張か待ち時間調整を検討。
- **no_std ビルド**: `cargo test --no-default-features --features embassy` で Embedded 用テストを通してから統合。
