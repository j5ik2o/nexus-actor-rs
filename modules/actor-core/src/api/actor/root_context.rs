use crate::runtime::message::DynMessage;
use crate::runtime::system::InternalRootContext;
use crate::{ActorRef, MailboxFactory, PriorityEnvelope, Props};
use alloc::boxed::Box;
use core::future::Future;
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError};

use super::{ask_with_timeout, AskFuture, AskResult, AskTimeoutFuture};

/// ルートアクターを操作するためのコンテキスト。
///
/// アクターシステムのトップレベルからアクターの生成やメッセージ送信を行います。
/// ガーディアン戦略を通じて、子アクターの障害処理を管理します。
pub struct RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>, {
  pub(crate) inner: InternalRootContext<'a, DynMessage, R, Strat>,
  pub(crate) _marker: PhantomData<U>,
}

impl<'a, U, R, Strat> RootContext<'a, U, R, Strat>
where
  U: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
  Strat: crate::api::guardian::GuardianStrategy<DynMessage, R>,
{
  /// 指定されたプロパティを使用して新しいアクターを生成します。
  ///
  /// # 引数
  ///
  /// * `props` - アクターの生成に使用するプロパティ
  ///
  /// # 戻り値
  ///
  /// 生成されたアクターへの参照、またはメールボックスエラー
  pub fn spawn(&mut self, props: Props<U, R>) -> Result<ActorRef<U, R>, QueueError<PriorityEnvelope<DynMessage>>> {
    let (internal_props, supervisor_cfg) = props.into_parts();
    let actor_ref = self
      .inner
      .spawn_with_supervisor(Box::new(supervisor_cfg.into_supervisor()), internal_props)?;
    Ok(ActorRef::new(actor_ref))
  }

  /// 指定されたアクターにメッセージを送信し、応答を待つ Future を返します。
  ///
  /// # 引数
  ///
  /// * `target` - メッセージの送信先アクター
  /// * `message` - 送信するメッセージ
  ///
  /// # 戻り値
  ///
  /// 応答を受け取るための Future、またはエラー
  pub fn request_future<V, Resp>(&self, target: &ActorRef<V, R>, message: V) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element, {
    target.request_future(message)
  }

  /// 指定されたアクターにメッセージを送信し、タイムアウト付きで応答を待つ Future を返します。
  ///
  /// # 引数
  ///
  /// * `target` - メッセージの送信先アクター
  /// * `message` - 送信するメッセージ
  /// * `timeout` - タイムアウトを示す Future
  ///
  /// # 戻り値
  ///
  /// タイムアウト付きで応答を受け取るための Future、またはエラー
  pub fn request_future_with_timeout<V, Resp, TFut>(
    &self,
    target: &ActorRef<V, R>,
    message: V,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    V: Element,
    Resp: Element,
    TFut: Future<Output = ()> + Unpin, {
    let future = target.request_future(message)?;
    Ok(ask_with_timeout(future, timeout))
  }

  /// すべてのメッセージをディスパッチします。
  ///
  /// # 戻り値
  ///
  /// 成功した場合は `Ok(())`、メールボックスエラーが発生した場合は `Err`
  ///
  /// # 非推奨
  ///
  /// バージョン 3.1.0 から非推奨です。代わりに `dispatch_next` または `run_until` を使用してください。
  #[deprecated(since = "3.1.0", note = "dispatch_next / run_until を使用してください")]
  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    #[allow(deprecated)]
    self.inner.dispatch_all()
  }

  /// 次のメッセージを1つディスパッチします。
  ///
  /// # 戻り値
  ///
  /// 成功した場合は `Ok(())`、メールボックスエラーが発生した場合は `Err`
  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.dispatch_next().await
  }
}
