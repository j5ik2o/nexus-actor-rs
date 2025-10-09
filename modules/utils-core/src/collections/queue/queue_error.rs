/// キュー操作時に発生するエラー。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError<T> {
  /// キューが満杯で要素を追加できない。返却された値は追加しようとした要素。
  Full(T),
  /// キューへの提供操作が失敗。返却された値は提供しようとした要素。
  OfferError(T),
  /// キューがクローズ済み。返却された値は送信しようとした要素。
  Closed(T),
  /// キューとの接続が切断された。
  Disconnected,
}
