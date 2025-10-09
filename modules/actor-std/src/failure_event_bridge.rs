use std::sync::Arc;

use nexus_actor_core_rs::{FailureEvent, FailureEventListener};
use tokio::sync::broadcast;

/// Tokio broadcast ベースで FailureEvent を配信するラッパ。
#[derive(Clone)]
pub struct TokioFailureEventBridge {
  sender: broadcast::Sender<FailureEvent>,
}

impl TokioFailureEventBridge {
  /// 新しい`TokioFailureEventBridge`インスタンスを作成します。
  ///
  /// # Arguments
  ///
  /// * `capacity` - 内部broadcastチャネルのバッファ容量
  ///
  /// # Returns
  ///
  /// 指定された容量のbroadcastチャネルを持つ新しいブリッジインスタンス
  pub fn new(capacity: usize) -> Self {
    let (sender, _) = broadcast::channel(capacity);
    Self { sender }
  }

  /// 障害イベントをブロードキャストするリスナーを取得します。
  ///
  /// このメソッドは、障害イベントを受け取り、すべての購読者にブロードキャストする
  /// `FailureEventListener`を返します。送信エラーは無視されます。
  ///
  /// # Returns
  ///
  /// 障害イベントをブロードキャストするリスナー関数
  pub fn listener(&self) -> FailureEventListener {
    let sender = self.sender.clone();
    Arc::new(move |event: FailureEvent| {
      let _ = sender.send(event.clone());
    })
  }

  /// 障害イベントの受信者を作成します。
  ///
  /// このメソッドは、ブロードキャストチャネルの新しい受信者を作成します。
  /// 返された受信者を使用して、ブリッジを通じてブロードキャストされた
  /// 障害イベントを受信できます。
  ///
  /// # Returns
  ///
  /// 障害イベントを受信するためのbroadcast受信者
  pub fn subscribe(&self) -> broadcast::Receiver<FailureEvent> {
    self.sender.subscribe()
  }
}
