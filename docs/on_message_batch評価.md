評価

- 現状の `on_message_batch` は Go 版の分岐構造を踏襲しているものの、`deserialize_any` を型推測のたびに呼び出し、その結果を `std::any::type_name::<T>()` で突き合わせる実装になっており、特別扱いのメッセージが増えるほど判定ロジックが肥大化しています (remote/src/endpoint_reader.rs:125-209, remote/src/message_decoder.rs)。
- Go 版は `Deserialize(typeName, serializerID)` が `interface{}` を返しスイッチ一発で振り分けられますが、Rust 版では `Arc<dyn Any>` からの `downcast_ref` を何度も試す必要があり、`type_names` で得られているワイヤ名を十分に活かし切れていません。`std::any::type_name` の結果はビルド環境差異や将来の内部表記変更で崩れるリスクもあります。

改善の方向性

1. 型名レジストリの導入
    - `serializer.rs` が持つ `DashMap<String, Arc<dyn SerializerAny>>` を活用し、`type_names[envelope.type_id]` をキーに一度だけ `deserialize_any` を呼ぶフローへ変更する。戻り値は `enum DecodedMessage` のような型に正規化し、特別扱いメッセージも `downcast_ref` に一本化する。
    - `remote/src/cluster/messages.rs` の `todo!()` になっている `RootSerialized` 実装を埋めれば、Go 版同様に「ワイヤ形式 ⇆ ローカル構造体」の変換責務を serializer 側へ寄せられ、`on_message_batch` 内の三段デコードは不要になる。
2. メッセージ種別ディスパッチの整理
    - Go 版 `remote/endpoint_reader.go:onMessageBatch` は `switch typeName` で Terminated/System/User を先に振り分けている。Rust 版も `message_type_name` または `serializer_id` の組み合わせで早期分岐すれば、`std::any::type_name::<T>()` との手探り比較を排除できる。型名は proto 層で固定化し、送信側と共有する `const` を導入する。
    - プロトコルバッファ経由のメッセージは `SerializerId::Proto` を強制し、proto 系メッセージは `deserialize_message` が `Arc<dyn Message>` を返すよう統一することで、Terminated 系の例外処理を減らせる。
3. 構造化した戻り値型の導入
    - `deserialize_any` の戻り値をそのまま `Arc<dyn Any + Send + Sync>` で扱うのではなく、`System`, `RemoteDeliver`, `PubSub` などの `enum Deserialized` に変換するヘルパーを設ける。`on_message_batch` はその enum を `match` するだけにすれば、分岐ごとに `Any` へ戻って再解釈する必要がなくなる。
4. ログとエラー処理の整理
    - 現状はシリアライズ失敗ごとに `info!`/`debug!` が大量に出力され、最終的に `None` なら汎用デコードへフォールバックする構造になっている。型判定を一度で済ませればログ出力を抑制でき、失敗時には即座に `EndpointReaderError::Deserialization` を返せるため、バッチ全体の制御フローが明瞭になる。

具体的アクションアイテム

- `remote/src/serializer.rs` に type_name → deserializer のルックアップヘルパーを追加し、`message_decoder.rs` からの直接 `type_name` 比較を排除する。
- `remote/src/message_decoder.rs` を `enum DecodedMessage` ベースのシンプルな `match` に書き換え、Terminated/Watch/Stop 系は専用の `System` バリアントへ寄せる。`protoactor-go/remote/endpoint_reader.go` のロジックを参考にしつつ Rust の `match` で表現する。
- `remote/src/cluster/messages.rs` の `RootSerialized` 実装を整理し、PubSub / Deliver 系のデシリアライズを serializer 側に寄せる。これにより `on_message_batch` はユニットテストで簡単に検証可能なフローに縮約される。
- 既存の `message_decoder` 単体テストに加えて、`EndpointReader::on_message_batch` の結合テストを追加し、Terminated と通常メッセージが混在するケース、未知の `type_id`/`serializer_id` を含むケースをカバーする。

検証観点

- `cargo test -p remote message_decoder::tests` と `cargo test -p remote endpoint_reader::tests` を新設し、Terminated/System/User の三系統が意図通りに振り分けられることを確認する。
- 送受信双方の型名が一致しているかを Go 実装のフィクスチャと比較し、`type_names` のズレでデシリアライズ失敗が発生しないかを検証する。
- 既存の `remote` クレート全体テストと `cargo clippy --workspace --all-targets` を通し、ログ出力のボリュームが期待通りに減ったことを `tracing` のフィルタで確認する。

まとめ

- Rust で Go のダイナミックディスパッチを模倣するのではなく、「ワイヤ名 → デシリアライズ → ローカル処理」の責務を段階的に切り分けることで `on_message_batch` の再デシリアライズ地獄から脱却できる。型名レジストリと `RootSerialized` を整備し、`DecodedMessage` のような中間表現を導入するのが改善の近道です。
