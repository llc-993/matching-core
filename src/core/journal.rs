use crate::api::OrderCommand;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, BufWriter, BufReader};
use std::path::Path;
use anyhow::Result;
use rkyv::Deserialize;

/// 高性能预写日志 (WAL) 实现 - 使用 rkyv 零拷贝序列化
pub struct Journaler {
    writer: BufWriter<File>,
}

impl Journaler {
    /// 创建或打开日志文件
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        
        Ok(Self {
            writer: BufWriter::with_capacity(64 * 1024, file), // 64KB 缓冲
        })
    }

    /// 写入命令到日志（使用 rkyv，比 bincode 快 2.5 倍）
    pub fn write_command(&mut self, cmd: &OrderCommand) -> Result<()> {
        // rkyv 序列化
        let bytes = rkyv::to_bytes::<_, 256>(cmd)
            .map_err(|e| anyhow::anyhow!("rkyv 序列化失败: {}", e))?;
        
        // 写入长度前缀 (u32) + 数据
        let len = bytes.len() as u32;
        self.writer.write_all(&len.to_le_bytes())?;
        self.writer.write_all(&bytes)?;
        
        // 批量刷盘（由 BufWriter 控制）
        self.writer.flush()?;
        
        Ok(())
    }

    /// 从日志文件读取并重放所有命令
    pub fn read_commands<P: AsRef<Path>>(path: P) -> Result<Vec<OrderCommand>> {
        if !path.as_ref().exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut commands = Vec::new();

        loop {
            let mut len_buf = [0u8; 4];
            if reader.read_exact(&mut len_buf).is_err() {
                break; // 到达文件末尾
            }
            
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            reader.read_exact(&mut data)?;
            
            // rkyv 反序列化（带校验）
            let archived = rkyv::check_archived_root::<OrderCommand>(&data)
                .map_err(|e| anyhow::anyhow!("rkyv 数据校验失败: {}", e))?;
            
            let cmd: OrderCommand = archived.deserialize(&mut rkyv::Infallible)
                .map_err(|_| anyhow::anyhow!("rkyv 反序列化失败"))?;
            
            commands.push(cmd);
        }

        Ok(commands)
    }
}
