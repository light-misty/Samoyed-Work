//! SessionCompaction:上下文压缩策略
//! 当上下文接近溢出时,压缩旧消息为摘要,保留最近消息和关键信息

use crate::config::app_settings::CompactionConfig;
use crate::errors::CommandError;
use crate::models::llm::ChatMessage;
use crate::services::llm::provider::LlmProvider;

/// 上下文压缩器
/// 在上下文接近溢出时,将旧消息压缩为摘要,保留最近消息和关键信息
pub struct ContextCompactor {
    /// 压缩配置
    config: CompactionConfig,
}

/// 压缩结果
pub struct CompactionResult {
    /// 压缩后的消息列表(包含摘要 system 消息 + 最近消息)
    pub messages: Vec<ChatMessage>,
    /// 压缩摘要(可注入到系统提示词)
    pub compaction_summary: String,
    /// 压缩前 token 数(由调用方设置)
    pub tokens_before: u64,
    /// 压缩后 token 数(由调用方设置)
    pub tokens_after: u64,
    /// 是否实际执行了压缩
    pub compacted: bool,
}

impl ContextCompactor {
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// 检查是否需要压缩
    /// 当当前 token 数超过上下文窗口的 trigger_threshold 比例时触发
    pub fn should_compact(&self, current_tokens: u64, context_window: u64) -> bool {
        if !self.config.enabled {
            return false;
        }
        let threshold = (context_window as f32 * self.config.trigger_threshold) as u64;
        current_tokens >= threshold
    }

    /// 执行上下文压缩
    /// 1. 保留最近 N 条消息
    /// 2. 对旧消息生成摘要(通过 LLM)
    /// 3. 对旧工具输出做 prune(截断)
    pub async fn compact(
        &self,
        messages: &[ChatMessage],
        llm_provider: &dyn LlmProvider,
    ) -> Result<CompactionResult, CommandError> {
        let keep_recent = self.config.keep_recent_messages;

        // 若消息数不超过保留数,不压缩
        if messages.len() <= keep_recent {
            log::debug!(
                "消息数 {} 不超过保留数 {},无需压缩",
                messages.len(),
                keep_recent
            );
            return Ok(CompactionResult {
                messages: messages.to_vec(),
                compaction_summary: String::new(),
                tokens_before: 0,
                tokens_after: 0,
                compacted: false,
            });
        }

        // 分割消息:old_messages + recent_messages
        let split_point = messages.len() - keep_recent;
        let old_messages = &messages[..split_point];
        let mut recent_messages: Vec<ChatMessage> = messages[split_point..].to_vec();

        log::info!(
            "上下文压缩:总消息数 {},旧消息数 {},保留最近 {} 条",
            messages.len(),
            old_messages.len(),
            keep_recent
        );

        // 对旧消息的工具输出做 prune(减少摘要请求的 token 消耗)
        let mut old_messages_pruned: Vec<ChatMessage> = old_messages.to_vec();
        if self.config.compact_tool_outputs {
            for msg in &mut old_messages_pruned {
                self.prune_tool_output(msg);
            }
        }

        // 对旧消息生成摘要
        let summary = self
            .generate_summary(&old_messages_pruned, llm_provider)
            .await?;

        // 对 recent_messages 中的工具输出做 prune
        if self.config.compact_tool_outputs {
            for msg in &mut recent_messages {
                self.prune_tool_output(msg);
            }
        }

        // 构建结果:摘要 system 消息 + recent_messages
        // ChatMessage 无 Default impl,手动填全所有字段
        let summary_system_msg = ChatMessage {
            role: "system".to_string(),
            content: format!("[Context Compaction Summary]\n{}", summary),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        };

        let mut result_messages = Vec::with_capacity(recent_messages.len() + 1);
        result_messages.push(summary_system_msg);
        result_messages.extend(recent_messages);

        log::info!(
            "上下文压缩完成:摘要长度 {} 字符,结果消息数 {}",
            summary.chars().count(),
            result_messages.len()
        );

        Ok(CompactionResult {
            messages: result_messages,
            compaction_summary: summary,
            tokens_before: 0,
            tokens_after: 0,
            compacted: true,
        })
    }

    /// 生成旧消息的摘要
    /// 通过 LLM 提取对话中的关键信息(用户需求/已完成工作/未完成任务/关键决策/文件路径/障碍)
    async fn generate_summary(
        &self,
        messages: &[ChatMessage],
        llm_provider: &dyn LlmProvider,
    ) -> Result<String, CommandError> {
        // 摘要请求的系统提示词(英文,提取关键信息)
        let system_prompt =
            "You are a conversation summarization assistant. Carefully read the following conversation history and extract the following key information:\n\n\
1. The user's core requirements and goals\n\
2. Completed work and results\n\
3. Unfinished tasks and pending items\n\
4. Key decisions and rationale\n\
5. File paths and important data involved\n\
6. Obstacles encountered and solutions\n\n\
Requirements:\n\
- Output a concise structured summary in English\n\
- Preserve all key file paths, function names, variable names and other technical details\n\
- Preserve important code snippets and data\n\
- Do not omit any key information to facilitate subsequent conversation reference";

        // 构建摘要请求消息:系统提示 + 旧消息
        let mut summary_messages = Vec::with_capacity(messages.len() + 1);
        summary_messages.push(ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        });
        summary_messages.extend(messages.iter().cloned());

        // 调用 LLM 生成摘要
        // LlmProvider::chat 签名为 (messages, tools),无 max_tokens 覆盖参数
        // 传空工具切片 &[] 表示不使用工具
        let response = llm_provider.chat(&summary_messages, &[]).await?;

        // 从 ChatResponse 中提取生成的文本内容
        // ChatResponse.choices[0].message.content 即为 LLM 生成的摘要
        let summary = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        if summary.is_empty() {
            log::warn!("上下文压缩:LLM 返回的摘要为空");
        }

        Ok(summary)
    }

    /// 对工具输出做 prune(截断过长的内容)
    /// ChatMessage 无 tool_result 字段,检查 role 是否为 "tool"(字符串比较)
    /// 若 content 长度超过 tool_output_max_chars,截断并追加截断标记
    fn prune_tool_output(&self, msg: &mut ChatMessage) {
        // 检查 role 是否为 "tool"(字符串比较,非枚举)
        if msg.role != "tool" {
            return;
        }

        let max_chars = self.config.tool_output_max_chars;
        let char_count = msg.content.chars().count();

        if char_count <= max_chars {
            return;
        }

        // 按字符边界安全截断(避免 UTF-8 截断 panic)
        let truncated: String = msg.content.chars().take(max_chars).collect();
        msg.content = format!(
            "{}\n...[truncated, original length {} chars]",
            truncated, char_count
        );

        log::debug!(
            "工具输出截断:原始 {} 字符 -> 保留 {} 字符",
            char_count,
            max_chars
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 token 数低于阈值时不应触发压缩
    #[test]
    fn test_should_compact_below_threshold() {
        let config = CompactionConfig {
            enabled: true,
            trigger_threshold: 0.8,
            keep_recent_messages: 10,
            keep_system_tokens: 4000,
            compact_tool_outputs: true,
            tool_output_max_chars: 2000,
        };
        let compactor = ContextCompactor::new(config);
        // 7999 < 8000(阈值),不应触发压缩
        assert!(!compactor.should_compact(7999, 10000));
    }

    /// 测试 token 数恰好等于阈值时应触发压缩
    #[test]
    fn test_should_compact_at_threshold() {
        let config = CompactionConfig {
            enabled: true,
            trigger_threshold: 0.8,
            keep_recent_messages: 10,
            keep_system_tokens: 4000,
            compact_tool_outputs: true,
            tool_output_max_chars: 2000,
        };
        let compactor = ContextCompactor::new(config);
        // 8000 == 8000(阈值),应触发压缩
        assert!(compactor.should_compact(8000, 10000));
    }

    /// 测试 token 数高于阈值时应触发压缩
    #[test]
    fn test_should_compact_above_threshold() {
        let config = CompactionConfig {
            enabled: true,
            trigger_threshold: 0.8,
            keep_recent_messages: 10,
            keep_system_tokens: 4000,
            compact_tool_outputs: true,
            tool_output_max_chars: 2000,
        };
        let compactor = ContextCompactor::new(config);
        // 9000 > 8000(阈值),应触发压缩
        assert!(compactor.should_compact(9000, 10000));
    }

    /// 测试禁用压缩时无论 token 数多大都不应触发
    #[test]
    fn test_should_compact_disabled() {
        let config = CompactionConfig {
            enabled: false,
            trigger_threshold: 0.8,
            keep_recent_messages: 10,
            keep_system_tokens: 4000,
            compact_tool_outputs: true,
            tool_output_max_chars: 2000,
        };
        let compactor = ContextCompactor::new(config);
        // 即使 token 数远超阈值,禁用时也不触发
        assert!(!compactor.should_compact(99999, 10000));
    }

    /// 测试工具输出超过最大字符数时会被截断并追加标记
    #[test]
    fn test_prune_tool_output_truncates_long_content() {
        let config = CompactionConfig {
            enabled: true,
            trigger_threshold: 0.8,
            keep_recent_messages: 10,
            keep_system_tokens: 4000,
            compact_tool_outputs: true,
            tool_output_max_chars: 10,
        };
        let compactor = ContextCompactor::new(config);
        let mut msg = ChatMessage {
            role: "tool".to_string(),
            content: "abcdefghijk".to_string(), // 11 个字符
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        };
        compactor.prune_tool_output(&mut msg);
        // 应包含截断标记
        assert!(
            msg.content.contains("truncated, original length 11 chars"),
            "应包含截断标记,实际内容: {}",
            msg.content
        );
        // 应以截断后的前 10 个字符开头
        assert!(
            msg.content.starts_with("abcdefghij"),
            "应以截断后的前 10 个字符开头,实际内容: {}",
            msg.content
        );
    }

    /// 测试工具输出未超过最大字符数时保持不变
    #[test]
    fn test_prune_tool_output_keeps_short_content() {
        let config = CompactionConfig {
            enabled: true,
            trigger_threshold: 0.8,
            keep_recent_messages: 10,
            keep_system_tokens: 4000,
            compact_tool_outputs: true,
            tool_output_max_chars: 100,
        };
        let compactor = ContextCompactor::new(config);
        let mut msg = ChatMessage {
            role: "tool".to_string(),
            content: "short".to_string(), // 5 个字符
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        };
        compactor.prune_tool_output(&mut msg);
        // 内容应保持不变
        assert_eq!(msg.content, "short");
    }

    /// 测试非 tool 角色的消息不会被截断
    #[test]
    fn test_prune_tool_output_skips_non_tool_messages() {
        let config = CompactionConfig {
            enabled: true,
            trigger_threshold: 0.8,
            keep_recent_messages: 10,
            keep_system_tokens: 4000,
            compact_tool_outputs: true,
            tool_output_max_chars: 10,
        };
        let compactor = ContextCompactor::new(config);
        let long_content = "a".repeat(100);
        let mut msg = ChatMessage {
            role: "user".to_string(),
            content: long_content.clone(),
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
            attachments: None,
            metadata: None,
        };
        compactor.prune_tool_output(&mut msg);
        // 非 tool 消息内容应保持不变
        assert_eq!(msg.content, long_content);
    }
}
