# Core最適化プラン第二弾

## 背景
- ActorContext 周辺のロック待ちによるレイテンシ増大が確認されつつある。
- PidSet や MessageHandles が複数の async ロックを抱えているためホットパスで await が多い。
- タイマー実装が tokio::time::Sleep を多重にラップしており再初期化コストが高い。

## TODO
- [x] tokio-console と tracing で ActorContext のロック待ち時間を計測し、ホットパスを特定する。 **(2025-09-26: docs/benchmarks/tracing_actor_context.md にログ反映)**
- [x] PidSet の同期ロック化または lock-free 化の PoC を作成し、既存実装とのベンチ比較を行う。 **(2025-09-26: parking_lot ベース同期化 + extras ロック分離)**
- [x] ReceiveTimeoutTimer を DelayQueue 等へ置き換える案を設計し、再初期化の計測手順をまとめる。 **(2025-09-26: receive_timeout_delayqueue PoC と baseline 計測)**
- [x] MessageHandles の MPSC キュー化やバッチ処理案を調査し、影響範囲と移行ステップを整理する。 **(2025-09-26: parking_lot::Mutex ベース同期化 + stash 非 async 化)**
- [x] RestartStatistics の lazy 初期化を OnceCell 等で代替する設計メモを用意し、ActorContext API の非同期依存を洗い出す。 **(2025-09-26: OnceCell 化 + extras lock リファクタ)**
- [x] ActorContextExtrasInner の読み取り専用データと可変データを分離し、ロックスコープ短縮の設計をまとめる。 **(2025-09-26: mutable 専用 InstrumentedRwLock + PidSet 内部同期)**
- [ ] ディスパッチャ経路のメトリクスを整備し、ActorContext 操作前後のキュー滞留時間を計測するベンチマークを追加する。 **(ベンチテンプレート作成済み)**
- [ ] core の変更が remote/cluster に及ぼす影響調査と移行計画のドラフトを作成する。 **(影響メモ・連携セクション追加済み)**

## 優先度と進行ステップ
1. ロック待ち計測 (tokio-console + tracing) をセットアップし、ActorContext 周辺の待機時間ヒートマップを取得する。
2. 計測結果に基づいて PidSet と ActorContextExtrasInner のロック分離案を PoC 化し、ベンチマーク比較を実施する。
3. タイマー周り (ReceiveTimeoutTimer) の再設計案をまとめ、DelayQueue 置換の実装スケッチとテスト戦略を記述する。
4. MessageHandles のキュー改善案を整理し、MPSC 化ベースラインを先に検証する。
5. RestartStatistics の初期化戦略を OnceCell 等で試作し、remote/cluster への影響調査メモを作成する。

## PoC 実装ロードマップ
1. ActorContext ロック待ち計測用の tracing/tokio-console セットアップを PoC ブランチで実行し、計測結果を docs/benchmarks/tracing_actor_context.md (仮) に記録する。
2. PidSet 同期化 PoC を core に限定して実施し、メトリクスおよびベンチ結果を比較。影響が大きければ remote 側への展開計画を更新する。
3. MessageHandles の MPSC 化 PoC を行い、メールボックスとの統合テストを追加。必要であれば inbox バックプレッシャーの挙動も検証する。
4. ReceiveTimeoutTimer の DelayQueue 置換をサンプルとテストで検証し、既存 API への影響度を整理する。
5. RestartStatistics 初期化方針の変更を PoC で評価し、OnceCell 等を活用した同期化案を提示する。

## リスクと課題
- tracing/tokio-console を導入することでビルド時間やバイナリサイズが増大する可能性があるため、feature flag での制御を徹底する。
- PidSet の同期ロック化により、既存の非同期タスクがブロッキングするリスクがある。Tokio の current_thread ランタイム利用時にデッドロックしないか検証が必要。
- MessageHandles の構造変更がメールボックス実装に波及し、送受信の公平性や順序保証を崩す危険がある。テストとドキュメントの整備が必須。
- DelayQueue 置換は tokio バージョン依存が強く、将来のアップデートで破壊的変更が発生するリスクがある。サードパーティ依存の棚卸しを行う。
- RestartStatistics の API 変更に伴い、外部クレートが依存している場合は破壊的変更になる。公開 API の変更範囲を明確化して事前告知する。

## ステークホルダー連携メモ
- core チーム: ActorContextExtras 再設計と PidSet/MessageHandles 同期化 PoC を主導し、進捗を週次で共有する。
- remote チーム: EndpointWatcher API 変更とリモート通信への影響を評価し、必要な調整をフィードバックする。
- cluster チーム: Gossip/Member 管理まわりでの依存有無を確認し、破壊的変更があれば事前にテストケースを準備する。
- QA/テスト担当: 新規ベンチや DelayQueue 置換シナリオを回帰テストスイートへ組み込む計画を作成する。
- PM/プロダクト: マイルストーンとリスクを把握し、必要なリソースやスケジュール調整を行う。

## 成果物 (ドキュメント/コード)
- docs/benchmarks/tracing_actor_context.md (仮): tracing/tokio-console 計測結果と分析サマリ
- docs/benchmarks/core_actor_context_lock.md (仮): PidSet/MessageHandles 同期化ベンチマークの比較表
- docs/design/actor_context_extras_sync.md (仮): ActorContextExtras 再設計ドラフトの詳細、Immutable/Mutable 分割案を含む
- docs/design/pid_set_sync_transition.md (仮): PidSet 同期化に伴う API 変更・テスト移行計画
- core/examples/receive_timeout_delayqueue.rs (仮): DelayQueue 置換サンプル
- bench/actor_context_lock.rs (仮): ActorContext ロック挙動を計測するベンチ

## マイルストーン案
- M1: tracing/tokio-console PoC 完了、ヒートマップ共有 (2 週間目)
- M2: PidSet/MessageHandles 同期化 PoC とテストリファクタ完了 (4 週間目)
- M3: ReceiveTimeoutTimer DelayQueue 置換 PoC とベンチ結果共有 (6 週間目)
- M4: RestartStatistics / ActorContextExtras 再設計ドラフト確定、remote/cluster 影響評価完了 (8 週間目)
- M5: 各最適化項目の実装可否判断と正式ロードマップ策定 (10 週間目)

## 初動タスクメモ
- tracing 設定ファイルを core/.tracing/actor_context.toml (仮) として作成し、計測対象とするスパンを定義する案を検討。
- bench/actor_context_lock.rs (仮) を追加する構成を検討し、既存ベンチとの差分を洗い出す。
- DelayQueue 検証用に core/examples/receive_timeout_delayqueue.rs (仮) を作成して動作確認する手順をまとめる。
- rg ActorContextExtras -g *.rs で remote/cluster 側の参照箇所を抽出し、影響範囲リストを別途作成する。

## tracing計測準備ノート
- tokio-console を dev-dependencies に追加する際は core/Cargo.toml を対象にし、feature flag 切り替えで本番コードに影響させない方針を検討する。
- tracing_subscriber 設定サンプルを core/.tracing/actor_context.toml (仮) に下記テンプレートで用意する。
  - layer = console_subscriber::ConsoleLayer::builder().with_default_env().spawn()
  - filter = 'nexus_actor=trace,actor_context=trace'
- 計測時は 'RUSTFLAGS='--cfg tokio_unstable'' を付与して tokio-console を有効化する運用を想定し、Makefile.toml に専用タスクを追加する案を評価する。
- ActorContext の await ポイントに tracing::instrument を入れる PoC を作成し、計測リリースビルドとの差異を記録する。必要に応じて feature gate で制御する。
- 取得したスパンデータを flamegraph 化するため、tracing-flame の導入可否を調査し、既存の coverage / bench フローとの干渉を確認する。

## ベンチ/DelayQueue検討ノート
- bench/actor_context_lock.rs (仮) では以下を計測対象とする案：
  - ActorContext::ensure_extras 経由のロック初期化パス
  - MessageHandles push/pop のホットパス
  - ReceiveTimeoutTimer 初期化およびリセット処理
- ベンチマーク実行時に 'cargo bench --bench actor_context_lock -- --sampling-mode Flat' を使い、ロック待ちが集中する区間を可視化する。
- DelayQueue 導入検証では 'tokio_util::time::DelayQueue' を用い、現在の SleepContainer との差分を測定する micro benchmark を作成する。
- DelayQueue 検証用サンプル (core/examples/receive_timeout_delayqueue.rs) において、アクターが receive_timeout を複数回再設定するケースを再現し、パフォーマンスと挙動の差分を記録する。
- bench 結果は docs/benchmarks/core_actor_context_lock.md (仮) に記録し、リグレッション検知のためのしきい値設定を検討する。

## ActorContextExtras再設計ノート
- 読み取り専用データ (children, watchers, extensions など) と可変データ (stash, receive_timeout_timer, restart_stats) のロック粒度を分離する案を比較する。
- RwLock の代替として parking_lot::RwLock や crossbeam 系の Atomic 構造を組み合わせる PoC パターンを検討。
- 内部で使用する PidSet や MessageHandles の API が非同期依存を持つため、同期ロック化する際の API 変更影響を整理する。
- 旧 API を保持する互換ラッパーの必要性を評価し、移行フェーズでの feature flag 追加案を検討する。
- デッドロック防止のため、await を跨ぐロングスパンのロック取得箇所を列挙し、設計時に優先的に解消する。

### 設計ドラフト草案
- ActorContextExtrasInner を 'InnerImmutable' ('children', 'watchers', 'extensions') と 'InnerMutable' ('stash', 'receive_timeout_timer', 'restart_stats') に分割し、'Arc<InnerImmutable>' + 'Mutex<InnerMutable>' 構成を検討する。
- 'PidSet' と 'MessageHandles' を同期ロックベースに置き換える場合、メソッドシグネチャから 'async' を排す必要があるため、影響範囲 (tests, core/actor/context 配下) を列挙する。
- 'RestartStatistics' の取得 API は lazy 初期化の代わりに 'Option<Arc<RestartStatistics>>' を返し、呼び出し側で lazy 化する設計変更案をまとめる。
- 旧構造との互換性検証として、'feature = 'legacy_context_extras'' を導入し、既存コードを段階的に移行できるか検討する。
- Lock-free 実装 (例: crossbeam::queue) を採用する場合のメリット/デメリットを簡潔にまとめ、PoC の優先順位を決める。

## PidSet / MessageHandles 影響洗い出し
- PidSet 利用箇所一覧:
  - core/src/actor/core/pid_set.rs および tests.rs (型本体と単体テスト)
  - core/src/actor/context/actor_context_extras.rs (children, watchers)
  - remote/src/endpoint_watcher.rs と tests.rs (DashMap と組み合わせた監視リスト)
- MessageHandles 利用箇所一覧:
  - core/src/actor/context/actor_context_extras.rs (stash 用)
  - core/src/actor/message/message_handles.rs (型本体)
- 非 async 化を行う場合、remote/src/endpoint_watcher.rs の async API 変更も必要になるため、remote チームと連携して変更順序を調整する。
- PidSet の内部を同期ロック化した場合、'await' を前提とする既存テスト (core/src/actor/core/pid_set/tests.rs) をリファクタリングする必要がある。

## remote endpoint watcher 調整案
- PidSet 同期化に合わせて 'EndpointWatcher' の内部キャッシュ ('DashMap<String, PidSet>') を同期ロック前提に書き換える案を検討する。
- 現行では 'PidSet::new().await' を利用しているため、同期化後は初期化関数を同期メソッドに変更し、非同期呼び出しを排除する。
- 監視対象追加/削除 ('add_watched', 'remove_watched') の API が 'async' である理由を再確認し、同期メソッド化しても競合が発生しないか調査する。
- 'EndpointWatcher' テスト (remote/src/endpoint_watcher/tests.rs) で await を前提にしたシナリオを書き換え、同期化後の動作保証を追加する。

## PidSet テストリファクタ計画
- 'core/src/actor/core/pid_set/tests.rs' の各テストを同期 API でも動作するように整理し、tokio runtime 依存を減らす。
- 非同期専用の挙動 (例: 同期待ち) を確認したいテストは '#[tokio::test(flavor = 'current_thread')]' など最小構成に切り替える。
- 同期化後に追加される新 API 用のユニットテストを準備し、'ensure_init', 'add', 'remove' などの挙動を同期的に検証する。
- ベンチマーク用に 'benchmarks/' へ PidSet 操作の micro benchmark を追加する案も検討する。

## remote/cluster影響メモ
- cluster モジュールの Gossip や Member 管理で ActorContextExtras を直接参照する箇所は現状無し。core 改修時は props/actor_context 経由の API 互換性を確認する。
- remote モジュールでコンテキスト情報が必要な箇所 ('remote/src/endpoint') を確認し、ActorContextExtras に依存しない設計にできるか検討する。
- remote/src/endpoint_watcher.rs の PidSet API を同期版へ移行済み。watch/unwatch ループでの await を削減し、ダッシュマップ操作のみで完結するよう改修した。
- core のロック構造変更に伴うメッセージ処理遅延がリモート通信に与える影響をベンチマークで追跡する仕組みを整える。
- cluster 側の PidSet 直接利用はなし。今後 ActorContextExtras の OnceCell 化が波及した場合は GossipState への参照キャッシュに留意する。
- 影響範囲を docs/core_optimization_plan.md に継続して追記し、モジュール横断のタスクは cluster/remote チームと連携する。

## メモ
- TODO の進捗はこのファイルに追記すること。
- ベンチマークは bench/ 以下の既存ケースを流用し、不足分は追加する。
- 変更のたびに cargo test --workspace を実施し、回帰がないことを確認する。

## 影響範囲リスト (初回調査)
- core/src/actor/context/actor_context_extras.rs
- core/src/actor/context/actor_context.rs
- remote/cluster 以下での ActorContextExtras 参照は現時点で検出されず。
