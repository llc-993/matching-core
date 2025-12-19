use crate::api::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// 分组处理器 - 负责命令批处理和分组
pub struct GroupingProcessor {
    group_counter: AtomicU64,
    msgs_in_group_limit: usize,
}

impl GroupingProcessor {
    pub fn new(msgs_in_group_limit: usize) -> Self {
        Self {
            group_counter: AtomicU64::new(0),
            msgs_in_group_limit,
        }
    }

    /// 处理命令，分配 events_group
    pub fn process(&self, cmd: &mut OrderCommand, msgs_in_current_group: &mut usize) {
        // 某些命令需要强制触发新组
        if matches!(
            cmd.command,
            OrderCommandType::Reset
                | OrderCommandType::PersistStateMatching
                | OrderCommandType::GroupingControl
        ) {
            self.group_counter.fetch_add(1, Ordering::SeqCst);
            *msgs_in_current_group = 0;
        }

        cmd.events_group = self.group_counter.load(Ordering::SeqCst);

        *msgs_in_current_group += 1;

        // 达到批次大小限制，切换到新组
        if *msgs_in_current_group >= self.msgs_in_group_limit {
            self.group_counter.fetch_add(1, Ordering::SeqCst);
            *msgs_in_current_group = 0;
        }
    }
}
