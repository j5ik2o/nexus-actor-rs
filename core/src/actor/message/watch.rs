// use std::fmt::Debug;
// use crate::actor::actor::Pid;
//
// #[derive(Clone, PartialEq)]
// pub struct Watch {
//   pub watcher: Option<Pid>,
// }
//
// impl Debug for Watch {
//   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//     write!(f, "Watch {{ watcher: {:?} }}", self.watcher.as_ref().map(|v| v.to_string()))
//   }
// }
//
// #[derive(Clone, PartialEq)]
// pub struct Unwatch {
//   pub watcher: Option<Pid>,
// }
//
// impl Debug for Unwatch {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "Unwatch {{ watcher: {:?} }}", self.watcher.as_ref().map(|v| v.to_string()))
//     }
// }
