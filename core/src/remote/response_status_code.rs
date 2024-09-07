use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(i32)]
pub enum ResponseStatusCode {
  ResponseStatusCodeOK = 0,
  ResponseStatusCodeUNAVAILABLE,
  ResponseStatusCodeTIMEOUT,
  ResponseStatusCodePROCESSNAMEALREADYEXIST,
  ResponseStatusCodeERROR,
  ResponseStatusCodeDeadLetter,
  ResponseStatusCodeMAX,
}
