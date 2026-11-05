//! 瞄准训练（W2-F，迁移 007）。训练本身全部在前端跑（Canvas），这里只负责：
//! - 成绩存取：`training_sessions` / `training_results`；
//! - 读取本机 CS2 配置里的 `sensitivity`。
//!
//! 命令的 JSON 结构见 `src/shared-types/src/training.ts`，说明见 `docs/training.md`。

pub mod cs2cfg;
pub mod commands;
pub mod repo;
