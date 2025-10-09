use nexus_utils_core_rs::{Element, PriorityMessage, DEFAULT_PRIORITY};

use crate::ActorId;
use crate::FailureInfo;

/// 制御メッセージかどうかを区別するチャネル種別。
///
/// メールボックス内でのメッセージの優先度制御に使用されます。
/// 制御メッセージは通常のメッセージよりも高い優先度で処理されます。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorityChannel {
  /// 通常のアプリケーションメッセージ
  Regular,
  /// システム制御メッセージ（停止、再起動など）
  Control,
}

/// protoactor-go の `SystemMessage` を参考にした制御メッセージ種別。
///
/// アクターシステムの内部制御に使用される特別なメッセージです。
/// フィールド構成は現段階の要件に絞り、必要に応じて拡張する方針です。
///
/// 各バリアントは異なる優先度を持ち、アクターライフサイクルの様々な局面で使用されます。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessage {
  /// 他のアクターを監視開始する
  Watch(ActorId),
  /// 他のアクターの監視を解除する
  Unwatch(ActorId),
  /// アクターの停止を指示する
  Stop,
  /// 障害の発生を通知する
  Failure(FailureInfo),
  /// アクターの再起動を指示する
  Restart,
  /// アクターのメッセージ処理を一時停止する
  Suspend,
  /// アクターのメッセージ処理を再開する
  Resume,
  /// 障害を親アクターへエスカレーションする
  Escalate(FailureInfo),
  /// 受信タイムアウトを通知する
  ReceiveTimeout,
}

impl SystemMessage {
  /// 推奨優先度を取得します。protoactor-go の優先度テーブルをベースに設定。
  ///
  /// # 戻り値
  /// メッセージの優先度（値が大きいほど高優先度）
  ///
  /// # 優先度の序列
  /// - Escalate: 最高優先度（DEFAULT_PRIORITY + 13）
  /// - Failure: DEFAULT_PRIORITY + 12
  /// - Restart: DEFAULT_PRIORITY + 11
  /// - Stop: DEFAULT_PRIORITY + 10
  /// - Suspend/Resume: DEFAULT_PRIORITY + 9
  /// - ReceiveTimeout: DEFAULT_PRIORITY + 8
  /// - Watch/Unwatch: DEFAULT_PRIORITY + 5
  pub fn priority(&self) -> i8 {
    match self {
      SystemMessage::Watch(_) | SystemMessage::Unwatch(_) => DEFAULT_PRIORITY + 5,
      SystemMessage::Stop => DEFAULT_PRIORITY + 10,
      SystemMessage::Failure(_) => DEFAULT_PRIORITY + 12,
      SystemMessage::Restart => DEFAULT_PRIORITY + 11,
      SystemMessage::Suspend | SystemMessage::Resume => DEFAULT_PRIORITY + 9,
      SystemMessage::Escalate(_) => DEFAULT_PRIORITY + 13,
      SystemMessage::ReceiveTimeout => DEFAULT_PRIORITY + 8,
    }
  }
}

impl Element for SystemMessage {}

/// Envelope型: 優先度付きメッセージを格納し、`PriorityMessage` を実装する。
///
/// メッセージを優先度やチャネル情報と共にラップします。
/// メールボックス内での優先度付きメッセージ処理に使用されます。
#[allow(dead_code)]
#[derive(Debug)]
pub struct PriorityEnvelope<M> {
  message: M,
  priority: i8,
  channel: PriorityChannel,
  system_message: Option<SystemMessage>,
}

#[allow(dead_code)]
impl<M> PriorityEnvelope<M> {
  /// 指定された優先度で通常チャネルのエンベロープを作成します。
  ///
  /// # 引数
  /// - `message`: ラップするメッセージ
  /// - `priority`: メッセージの優先度
  pub fn new(message: M, priority: i8) -> Self {
    Self::with_channel(message, priority, PriorityChannel::Regular)
  }

  /// 指定されたチャネルと優先度でエンベロープを作成します。
  ///
  /// # 引数
  /// - `message`: ラップするメッセージ
  /// - `priority`: メッセージの優先度
  /// - `channel`: メッセージのチャネル種別
  pub fn with_channel(message: M, priority: i8, channel: PriorityChannel) -> Self {
    Self {
      message,
      priority,
      channel,
      system_message: None,
    }
  }

  /// 制御チャネルのエンベロープを作成します。
  ///
  /// # 引数
  /// - `message`: ラップするメッセージ
  /// - `priority`: メッセージの優先度
  pub fn control(message: M, priority: i8) -> Self {
    Self::with_channel(message, priority, PriorityChannel::Control)
  }

  /// 内部のメッセージへの参照を取得します。
  pub fn message(&self) -> &M {
    &self.message
  }

  /// メッセージの優先度を取得します。
  pub fn priority(&self) -> i8 {
    self.priority
  }

  /// メッセージのチャネル種別を取得します。
  pub fn channel(&self) -> PriorityChannel {
    self.channel
  }

  /// 制御メッセージかどうかを判定します。
  ///
  /// # 戻り値
  /// 制御チャネルの場合は `true`、通常チャネルの場合は `false`
  pub fn is_control(&self) -> bool {
    matches!(self.channel, PriorityChannel::Control)
  }

  /// 関連するシステムメッセージがあれば取得します。
  pub fn system_message(&self) -> Option<&SystemMessage> {
    self.system_message.as_ref()
  }

  /// エンベロープを分解してメッセージと優先度を取得します。
  ///
  /// # 戻り値
  /// `(メッセージ, 優先度)` のタプル
  pub fn into_parts(self) -> (M, i8) {
    (self.message, self.priority)
  }

  /// エンベロープを分解してメッセージ、優先度、チャネルを取得します。
  ///
  /// # 戻り値
  /// `(メッセージ, 優先度, チャネル)` のタプル
  pub fn into_parts_with_channel(self) -> (M, i8, PriorityChannel) {
    (self.message, self.priority, self.channel)
  }

  /// メッセージに関数を適用して新しい型のエンベロープに変換します。
  ///
  /// 優先度とチャネル情報は保持されます。
  ///
  /// # 引数
  /// - `f`: メッセージを変換する関数
  pub fn map<N>(self, f: impl FnOnce(M) -> N) -> PriorityEnvelope<N> {
    PriorityEnvelope {
      message: f(self.message),
      priority: self.priority,
      channel: self.channel,
      system_message: self.system_message,
    }
  }

  /// 優先度に関数を適用して変更します。
  ///
  /// # 引数
  /// - `f`: 優先度を変換する関数
  pub fn map_priority(mut self, f: impl FnOnce(i8) -> i8) -> Self {
    self.priority = f(self.priority);
    self
  }
}

#[allow(dead_code)]
impl<M> PriorityEnvelope<M>
where
  M: Element,
{
  /// デフォルト優先度で通常チャネルのエンベロープを作成します。
  ///
  /// # 引数
  /// - `message`: ラップするメッセージ
  pub fn with_default_priority(message: M) -> Self {
    Self::new(message, DEFAULT_PRIORITY)
  }
}

impl PriorityEnvelope<SystemMessage> {
  /// SystemMessage 用の Envelope ヘルパー。
  ///
  /// システムメッセージを自動的に適切な優先度と制御チャネルでラップします。
  ///
  /// # 引数
  /// - `message`: ラップするシステムメッセージ
  pub fn from_system(message: SystemMessage) -> Self {
    let priority = message.priority();
    let system_clone = message.clone();
    let mut envelope = PriorityEnvelope::with_channel(message, priority, PriorityChannel::Control);
    envelope.system_message = Some(system_clone);
    envelope
  }
}

impl<M> PriorityMessage for PriorityEnvelope<M>
where
  M: Element,
{
  fn get_priority(&self) -> Option<i8> {
    Some(self.priority)
  }
}

impl<M> Element for PriorityEnvelope<M> where M: Element {}
