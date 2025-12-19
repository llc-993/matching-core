use crate::api::*;
use crate::core::pipeline::Pipeline;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// 交易所核心配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub ring_buffer_size: usize,
    pub matching_engines_num: usize,
    pub risk_engines_num: usize,
    pub producer_type: ProducerType,
    pub wait_strategy: WaitStrategyType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ProducerType {
    Single,
    Multi,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WaitStrategyType {
    BusySpin,
    Yielding,
    Blocking,
    Sleeping,
}

impl ExchangeConfig {
    // 这里不再需要同步转换方法，因为 startup 内部已经处理了配置到具体类型的映射
}

#[derive(Serialize, Deserialize)]
pub struct ExchangeState {
    pub config: ExchangeConfig,
    pub pipeline_state: crate::core::pipeline::PipelineState,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            ring_buffer_size: 64 * 1024,
            matching_engines_num: 1,
            risk_engines_num: 1,
            producer_type: ProducerType::Single,
            wait_strategy: WaitStrategyType::BusySpin,
        }
    }
}

/// 结果消费者回调
pub type ResultConsumer = Arc<dyn Fn(&OrderCommand) + Send + Sync>;

use crate::core::journal::Journaler;
use std::path::Path;

use crate::core::snapshot::SnapshotStore;

/// 内部接口，用于类型抹除 Disruptor 的泛型 Producer
trait Publisher {
    fn publish(&mut self, cmd: OrderCommand);
}

struct ProducerWrapper<P: disruptor::Producer<OrderCommand>>(P);

impl<P: disruptor::Producer<OrderCommand>> Publisher for ProducerWrapper<P> {
    fn publish(&mut self, cmd: OrderCommand) {
        self.0.publish(|event| {
            *event = cmd;
        });
    }
}

/// 交易所核心
pub struct ExchangeCore {
    config: ExchangeConfig,
    // 使用 Publisher trait 对象隐藏具体的扰乱器生产者类型
    producer: Option<Box<dyn Publisher>>,
    pipeline: Option<Pipeline>,
    journaler: Option<Journaler>,
    snapshot_store: Option<SnapshotStore>,
}

impl ExchangeCore {
    pub fn new(config: ExchangeConfig) -> Self {
        let pipeline = Pipeline::new(&config);
        Self { 
            config, 
            pipeline: Some(pipeline),
            producer: None,
            journaler: None,
            snapshot_store: None,
        }
    }

    /// 启动 Disruptor 流水线
    pub fn startup(&mut self) {
        if self.producer.is_some() {
            return;
        }

        if let Some(mut pipeline) = self.pipeline.take() {
            let ring_size = self.config.ring_buffer_size;
            
            // 封装事件处理逻辑
            // Disruptor 3.6.1 的 handler 接收的是 &E (不可变)
            // 为了维持原有 Pipeline 的可变逻辑，我们在处理前进行克隆
            let handler = move |event: &OrderCommand, sequence: i64, end_of_batch: bool| {
                let mut cmd_mut = event.clone();
                pipeline.handle_event(&mut cmd_mut, sequence, end_of_batch);
            };

            // 使用 build_single_producer / build_multi_producer
            // 目前 3.6.1 仅显式支持 BusySpin 等几种策略在 wait_strategies 下
            let producer: Box<dyn Publisher> = match self.config.producer_type {
                ProducerType::Single => {
                    Box::new(ProducerWrapper(disruptor::build_single_producer(ring_size, || OrderCommand::default(), disruptor::wait_strategies::BusySpin)
                        .handle_events_with(handler)
                        .build()))
                },
                ProducerType::Multi => {
                    Box::new(ProducerWrapper(disruptor::build_multi_producer(ring_size, || OrderCommand::default(), disruptor::wait_strategies::BusySpin)
                        .handle_events_with(handler)
                        .build()))
                }
            };

            self.producer = Some(producer);
        }
    }

    /// 启用快照管理
    pub fn enable_snapshotting<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        self.snapshot_store = Some(SnapshotStore::new(path)?);
        Ok(())
    }

    /// 生成当前状态快照
    pub fn take_snapshot(&self, seq_id: u64) -> anyhow::Result<()> {
        if let Some(store) = &self.snapshot_store {
            let state = self.serialize_state();
            store.save_snapshot(&state, seq_id)?;
        }
        Ok(())
    }

    /// 加载最新的快照并恢复状态
    pub fn load_latest_snapshot(&mut self) -> anyhow::Result<bool> {
        if let Some(store) = &self.snapshot_store {
            if let Some(seq_id) = store.get_latest_seq_id()? {
                let state = store.load_snapshot(seq_id)?;
                *self = Self::from_state(state);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 启用日志持久化
    pub fn enable_journaling<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        self.journaler = Some(Journaler::new(path)?);
        Ok(())
    }

    /// 结果消费者回调
    pub fn set_result_consumer(&mut self, consumer: ResultConsumer) {
        if let Some(p) = &mut self.pipeline {
            p.set_result_consumer(consumer);
        }
    }

    pub fn add_symbol(&mut self, spec: CoreSymbolSpecification) {
        if let Some(p) = &mut self.pipeline {
            p.add_symbol(spec);
        }
    }

    /// 提交命令
    pub fn submit_command(&mut self, mut cmd: OrderCommand) -> OrderCommand {
        if let Some(j) = &mut self.journaler {
            let _ = j.write_command(&cmd);
        }
        
        if let Some(producer) = &mut self.producer {
            producer.publish(cmd.clone());
            cmd
        } else if let Some(pipeline) = &mut self.pipeline {
            pipeline.handle_event(&mut cmd, 0, true);
            cmd
        } else {
            panic!("ExchangeCore 未就绪");
        }
    }

    /// 从日志重放
    pub fn replay_journal<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let commands = Journaler::read_commands(path)?;
        for mut cmd in commands {
            if let Some(pipeline) = &mut self.pipeline {
                pipeline.handle_event(&mut cmd, 0, true);
            } else {
                self.submit_command(cmd);
            }
        }
        Ok(())
    }

    pub fn serialize_state(&self) -> ExchangeState {
        ExchangeState {
            config: self.config.clone(),
            pipeline_state: self.pipeline.as_ref().expect("只能在启动前序列化").serialize_state(),
        }
    }

    pub fn from_state(state: ExchangeState) -> Self {
        Self {
            config: state.config,
            pipeline: Some(Pipeline::from_state(state.pipeline_state)),
            producer: None,
            journaler: None,
            snapshot_store: None,
        }
    }
}

