use core::future::Future;

use nexus_utils_core_rs::{Element, QueueError, QueueRw, QueueSize};

use super::queue_mailbox::MailboxOptions;

/// メールボックスとプロデューサーのペア型エイリアス。
///
/// メールボックスの作成時に返される、受信側と送信側のハンドルのペアです。
pub type MailboxPair<Q, S> = (super::QueueMailbox<Q, S>, super::QueueMailboxProducer<Q, S>);

/// Mailbox abstraction that decouples message queue implementations from core logic.
///
/// メッセージキューの実装をコアロジックから分離する抽象化トレイトです。
/// 様々なキュー実装（有界/無界、優先度付きなど）を統一的に扱えるようにします。
///
/// # 型パラメータ
/// - `M`: 処理するメッセージの型
pub trait Mailbox<M> {
  /// メッセージ送信時のエラー型
  type SendError;

  /// メッセージ受信のFuture型
  type RecvFuture<'a>: Future<Output = Result<M, QueueError<M>>> + 'a
  where
    Self: 'a;

  /// メッセージの送信を試みます（ブロッキングなし）。
  ///
  /// # Arguments
  /// - `message`: 送信するメッセージ
  ///
  /// # Returns
  /// 成功時は `Ok(())`、失敗時は `Err(SendError)`
  fn try_send(&self, message: M) -> Result<(), Self::SendError>;

  /// メッセージを非同期的に受信します。
  ///
  /// # Returns
  /// メッセージ受信のFuture
  fn recv(&self) -> Self::RecvFuture<'_>;

  /// メールボックス内のメッセージ数を取得します。
  ///
  /// デフォルト実装では無制限を返します。
  fn len(&self) -> QueueSize {
    QueueSize::limitless()
  }

  /// メールボックスの容量を取得します。
  ///
  /// デフォルト実装では無制限を返します。
  fn capacity(&self) -> QueueSize {
    QueueSize::limitless()
  }

  /// メールボックスが空かどうかを判定します。
  ///
  /// # Returns
  /// 空の場合は `true`、メッセージがある場合は `false`
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  /// メールボックスを閉じます。
  ///
  /// デフォルト実装は何もしません。
  fn close(&self) {}

  /// メールボックスが閉じられているかどうかを判定します。
  ///
  /// デフォルト実装では常に `false` を返します。
  ///
  /// # Returns
  /// 閉じられている場合は `true`、開いている場合は `false`
  fn is_closed(&self) -> bool {
    false
  }
}

/// Notification primitive used by `QueueMailbox` to park awaiting receivers until
/// new messages are available.
///
/// メッセージ到着の通知に使用される同期プリミティブです。
/// 受信側がメッセージを待機し、送信側が到着を通知する仕組みを提供します。
pub trait MailboxSignal: Clone {
  /// 待機のFuture型
  type WaitFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  /// メッセージが到着したことを待機中の受信者に通知します。
  fn notify(&self);

  /// メッセージの到着を待機します。
  ///
  /// # Returns
  /// 通知を待つFuture
  fn wait(&self) -> Self::WaitFuture<'_>;
}

/// メールボックスを作成するファクトリートレイト。
///
/// 特定の非同期ランタイム（TokioやAsync-stdなど）に応じた
/// メールボックスとキューの実装を生成します。
pub trait MailboxFactory {
  /// 通知シグナルの型
  type Signal: MailboxSignal;

  /// メッセージキューの型
  type Queue<M>: QueueRw<M> + Clone
  where
    M: Element;

  /// 指定されたオプションでメールボックスを作成します。
  ///
  /// # Arguments
  /// - `options`: メールボックスの容量設定
  ///
  /// # Returns
  /// `(メールボックス, プロデューサー)` のペア
  fn build_mailbox<M>(&self, options: MailboxOptions) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element;

  /// デフォルト設定でメールボックスを作成します。
  ///
  /// # Returns
  /// `(メールボックス, プロデューサー)` のペア
  fn build_default_mailbox<M>(&self) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element, {
    self.build_mailbox(MailboxOptions::default())
  }
}
