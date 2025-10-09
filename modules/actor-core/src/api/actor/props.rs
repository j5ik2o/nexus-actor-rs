use alloc::rc::Rc;
use alloc::sync::Arc;

use crate::runtime::context::ActorContext;
use crate::runtime::message::{take_metadata, DynMessage};
use crate::runtime::system::InternalProps;
use crate::Supervisor;
use crate::SystemMessage;
use crate::{MailboxFactory, MailboxOptions, PriorityEnvelope};
use nexus_utils_core_rs::Element;

use super::behavior::SupervisorStrategyConfig;
use super::{ActorAdapter, Behavior, Context};
use crate::api::MessageEnvelope;
use core::cell::RefCell;
use core::marker::PhantomData;

/// アクター生成時の設定を保持するプロパティ。
///
/// アクターの振る舞い、メールボックス設定、スーパーバイザー戦略などを含む。
pub struct Props<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  inner: InternalProps<DynMessage, R>,
  _marker: PhantomData<U>,
  supervisor: SupervisorStrategyConfig,
}

impl<U, R> Props<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  /// メッセージハンドラを指定して新しい`Props`を作成する。
  ///
  /// # 引数
  /// * `options` - メールボックスオプション
  /// * `handler` - ユーザーメッセージを処理するハンドラ関数
  pub fn new<F>(options: MailboxOptions, handler: F) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static, {
    let handler_cell = Rc::new(RefCell::new(handler));
    Self::with_behavior(options, {
      let handler_cell = handler_cell.clone();
      move || {
        let handler_cell = handler_cell.clone();
        Behavior::stateless(move |ctx: &mut Context<'_, '_, U, R>, msg: U| {
          (handler_cell.borrow_mut())(ctx, msg);
        })
      }
    })
  }

  /// Behaviorファクトリを指定して新しい`Props`を作成する。
  ///
  /// # 引数
  /// * `options` - メールボックスオプション
  /// * `behavior_factory` - アクターの振る舞いを生成するファクトリ関数
  pub fn with_behavior<F>(options: MailboxOptions, behavior_factory: F) -> Self
  where
    F: Fn() -> Behavior<U, R> + 'static, {
    Self::with_behavior_and_system::<_, fn(&mut Context<'_, '_, U, R>, SystemMessage)>(options, behavior_factory, None)
  }

  /// ユーザーメッセージハンドラとシステムメッセージハンドラを指定して新しい`Props`を作成する。
  ///
  /// # 引数
  /// * `options` - メールボックスオプション
  /// * `user_handler` - ユーザーメッセージを処理するハンドラ関数
  /// * `system_handler` - システムメッセージを処理するハンドラ関数（オプション）
  pub fn with_system_handler<F, G>(options: MailboxOptions, user_handler: F, system_handler: Option<G>) -> Self
  where
    F: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, U) + 'static,
    G: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static, {
    let handler_cell = Rc::new(RefCell::new(user_handler));
    Self::with_behavior_and_system(
      options,
      {
        let handler_cell = handler_cell.clone();
        move || {
          let handler_cell = handler_cell.clone();
          Behavior::stateless(move |ctx: &mut Context<'_, '_, U, R>, msg: U| {
            (handler_cell.borrow_mut())(ctx, msg);
          })
        }
      },
      system_handler,
    )
  }

  /// Behaviorファクトリとシステムメッセージハンドラを指定して新しい`Props`を作成する。
  ///
  /// 最も柔軟な`Props`作成方法で、振る舞いとシステムメッセージハンドラの両方を指定できる。
  ///
  /// # 引数
  /// * `options` - メールボックスオプション
  /// * `behavior_factory` - アクターの振る舞いを生成するファクトリ関数
  /// * `system_handler` - システムメッセージを処理するハンドラ関数（オプション）
  pub fn with_behavior_and_system<F, S>(
    options: MailboxOptions,
    behavior_factory: F,
    system_handler: Option<S>,
  ) -> Self
  where
    F: Fn() -> Behavior<U, R> + 'static,
    S: for<'r, 'ctx> FnMut(&mut Context<'r, 'ctx, U, R>, SystemMessage) + 'static, {
    let behavior_factory: Arc<dyn Fn() -> Behavior<U, R> + 'static> = Arc::new(behavior_factory);
    let mut adapter = ActorAdapter::new(behavior_factory.clone(), system_handler);
    let map_system = ActorAdapter::<U, R>::create_map_system();
    let supervisor = adapter.supervisor_config();

    let handler = move |ctx: &mut ActorContext<'_, DynMessage, R, dyn Supervisor<DynMessage>>, message: DynMessage| {
      let Ok(envelope) = message.downcast::<MessageEnvelope<U>>() else {
        panic!("unexpected message type delivered to typed handler");
      };
      match envelope {
        MessageEnvelope::User(user) => {
          let (message, metadata_key) = user.into_parts();
          let metadata = metadata_key.and_then(take_metadata).unwrap_or_default();
          let mut typed_ctx = Context::with_metadata(ctx, metadata);
          adapter.handle_user(&mut typed_ctx, message);
        }
        MessageEnvelope::System(message) => {
          let mut typed_ctx = Context::new(ctx);
          adapter.handle_system(&mut typed_ctx, message);
        }
      }
    };

    let inner = InternalProps::new(options, map_system, handler);
    Self {
      inner,
      _marker: PhantomData,
      supervisor,
    }
  }

  /// 内部プロパティとスーパーバイザー設定に分解する（内部API）。
  ///
  /// # 戻り値
  /// `(InternalProps, SupervisorStrategyConfig)`のタプル
  pub(crate) fn into_parts(self) -> (InternalProps<DynMessage, R>, SupervisorStrategyConfig) {
    (self.inner, self.supervisor)
  }
}
