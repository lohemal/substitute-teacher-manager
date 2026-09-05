//! IPC 진입점.
//!
//! 여기서는 입력 검증과 DTO 변환만 하고, 실제 계산은 `domain`,
//! 실제 SQL은 `repo`에 맡긴다.

pub mod admin;
pub mod app;
pub mod assign;
pub mod bell;
pub mod find;
pub mod lesson;
pub mod priority;
pub mod school;
pub mod setup;
pub mod stats;
pub mod teacher;

pub use admin::*;
pub use app::*;
pub use assign::*;
pub use bell::*;
pub use find::*;
pub use lesson::*;
pub use priority::*;
pub use school::*;
pub use setup::*;
pub use stats::*;
pub use teacher::*;
