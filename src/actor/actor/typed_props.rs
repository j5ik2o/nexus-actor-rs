use crate::actor::actor::Props;
use crate::actor::message::Message;
use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct TypedProps<M: Message> {
  underlying: Props,
  phantom_data: PhantomData<M>,
}

impl<M: Message> TypedProps<M> {
  pub fn new(underlying: Props) -> Self {
    Self {
      underlying,
      phantom_data: PhantomData,
    }
  }

  pub fn get_underlying(&self) -> &Props {
    &self.underlying
  }
}

impl<M: Message> From<Props> for TypedProps<M> {
  fn from(props: Props) -> Self {
    Self::new(props)
  }
}

impl<M: Message> From<TypedProps<M>> for Props {
  fn from(typed_props: TypedProps<M>) -> Self {
    typed_props.underlying
  }
}
