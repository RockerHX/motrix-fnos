use crate::config::aria2::EngineGeneration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppCompatibility {
    pub engine_generation: EngineGeneration,
    pub mode: String,
    pub reason_code: Option<String>,
    pub unfinished_task_count: usize,
    pub unresolved_operation_count: usize,
    pub message: String,
}

impl AppCompatibility {
    pub fn normal(generation: EngineGeneration) -> Self {
        Self {
            engine_generation: generation,
            mode: "normal".to_string(),
            reason_code: None,
            unfinished_task_count: 0,
            unresolved_operation_count: 0,
            message: String::new(),
        }
    }

    pub(crate) fn restricted(generation: EngineGeneration, reason: &str) -> Self {
        Self {
            mode: "restricted".to_string(),
            reason_code: Some(reason.to_string()),
            message: "无法确认当前引擎或任务状态，下载功能暂不可用。请检查引擎版本并导出诊断。"
                .to_string(),
            ..Self::normal(generation)
        }
    }

    pub fn ensure_download_available(&self) -> Result<(), String> {
        if self.mode == "normal" {
            Ok(())
        } else {
            Err(format!("engine_upgrade_required：{}", self.message))
        }
    }
}
