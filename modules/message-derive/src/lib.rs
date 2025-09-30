#[allow(dead_code)]
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  let name = &input.ident;

  let expanded = quote! {
      impl Message for #name {
          fn eq_message(&self, other: &dyn Message) -> bool {
              other.as_any().downcast_ref::<Self>()
                  .map_or(false, |other| self == other)
          }

          fn as_any(&self) -> &(dyn std::any::Any + Send + Sync + 'static) {
              self
          }

          fn get_type_name(&self) -> String {
              std::any::type_name_of_val(self).to_string()
          }
      }
  };

  TokenStream::from(expanded)
}
