//! 同期プリミティブの基盤モジュール。
//!
//! このモジュールは、共有参照と状態セルの実装を提供します。
//! これらは、コレクションや並行処理プリミティブの基盤として使用されます。
//!
//! # 提供される型
//!
//! - **RcShared / ArcShared**: 共有参照ラッパー（`Shared` トレイトの実装）
//! - **RcStateCell / ArcStateCell**: 状態セル（`StateCell` トレイトの実装）
//!
//! # フィーチャフラグ
//!
//! - **`rc`**: `Rc` ベースの実装（シングルスレッド専用）
//! - **`arc`**: `Arc` ベースの実装（マルチスレッド対応）
//!   - `ArcLocal*`: ローカルミューテックスを使用した最適化実装
//!   - `ArcCs*`: クリティカルセクションベースの実装
//!   - `Arc*`: 標準的な実装

#[cfg(feature = "arc")]
mod arc;
#[cfg(feature = "rc")]
mod rc;

#[cfg(feature = "arc")]
pub use arc::{ArcCsStateCell, ArcLocalStateCell, ArcShared, ArcStateCell};
#[cfg(feature = "rc")]
pub use rc::{RcShared, RcStateCell};
