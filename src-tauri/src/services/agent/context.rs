use crate::models::llm::{ChatMessage, LlmToolCall};
use super::prompts::document_design::get_all_design_guides;

/// reasoning_content 压缩阈值（字符数），超过此长度的早期思考内容将被截断
const REASONING_COMPRESS_THRESHOLD: usize = 500;
/// 压缩后保留的字符数
const REASONING_COMPRESS_KEEP: usize = 200;

/// Agent 执行上下文
/// 管理对话历史和系统提示词
pub struct AgentContext {
    /// 会话 ID
    pub session_id: String,
    /// 对话消息历史
    pub messages: Vec<ChatMessage>,
    /// 系统提示词
    pub system_prompt: String,
    /// 最大迭代次数
    pub max_iterations: u32,
    /// 已持久化的消息数量，用于增量持久化
    persisted_count: usize,
    /// 当前工作区路径，用于 Skill 的路径安全校验
    pub workspace_path: String,
    /// 当前工作区 ID，用于版本快照等需要关联工作区的操作
    pub workspace_id: String,
}

impl AgentContext {
    pub fn new(session_id: String, system_prompt: String) -> Self {
        Self {
            session_id,
            messages: Vec::new(),
            system_prompt,
            max_iterations: 20,
            persisted_count: 0,
            workspace_path: String::new(),
            workspace_id: String::new(),
        }
    }

    /// 添加用户消息
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        });
    }

    /// 添加助手消息
    pub fn add_assistant_message(&mut self, content: &str, tool_calls: Option<Vec<LlmToolCall>>, reasoning_content: Option<String>) {
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
            tool_calls,
            tool_call_id: None,
            reasoning_content,
        });
    }

    /// 添加工具执行结果消息
    pub fn add_tool_result(&mut self, call_id: &str, content: &str) {
        self.messages.push(ChatMessage {
            role: "tool".to_string(),
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: Some(call_id.to_string()),
            reasoning_content: None,
        });
    }

    /// 获取包含系统提示词的完整消息列表
    pub fn get_messages(&self) -> Vec<ChatMessage> {
        let mut all = vec![ChatMessage {
            role: "system".to_string(),
            content: self.system_prompt.clone(),
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        }];
        all.extend(self.messages.clone());
        all
    }

    /// 获取针对指定迭代轮次优化后的消息列表
    /// 与 get_messages 的区别：
    /// 1. 压缩早期迭代的 reasoning_content（保留最近 1 轮完整，更早轮次截取前 200 字符）
    /// 2. 迭代 > 1 时在系统提示词后追加上下文提示，告知 LLM 这是继续推理
    pub fn get_messages_for_iteration(&self, current_iteration: u32) -> Vec<ChatMessage> {
        // 构建系统提示词，迭代 > 1 时追加继续推理提示
        let system_content = if current_iteration > 1 {
            format!(
                "{}\n\n注意：你正在继续执行之前的任务。以下是之前步骤的执行结果，请直接基于这些结果继续操作，无需重复之前的分析。",
                self.system_prompt
            )
        } else {
            self.system_prompt.clone()
        };

        let mut all = vec![ChatMessage {
            role: "system".to_string(),
            content: system_content,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        }];

        // 找出最后一条包含 reasoning_content 的 assistant 消息的索引
        let last_reasoning_idx = self.messages.iter().rposition(|m| {
            m.role == "assistant" && m.reasoning_content.is_some()
        });

        // 遍历消息，压缩早期的 reasoning_content
        for (i, msg) in self.messages.iter().enumerate() {
            let mut compressed_msg = msg.clone();

            if let Some(rc) = &msg.reasoning_content {
                // 判断是否为"最近一轮"的 reasoning_content
                let is_latest = last_reasoning_idx.is_none_or(|idx| i == idx);

                if !is_latest && rc.len() > REASONING_COMPRESS_THRESHOLD {
                    // 压缩早期的 reasoning_content：保留前 N 个字符 + 省略标记
                    let kept = rc.chars().take(REASONING_COMPRESS_KEEP).collect::<String>();
                    compressed_msg.reasoning_content = Some(format!("{}...(已省略)", kept));
                    log::debug!(
                        "压缩早期 reasoning_content: 原始长度={}, 压缩后长度={}, 消息索引={}",
                        rc.len(),
                        compressed_msg.reasoning_content.as_ref().unwrap().len(),
                        i
                    );
                }
            }

            all.push(compressed_msg);
        }

        all
    }

    /// 获取尚未持久化的消息列表（增量持久化用）
    /// 返回从 persisted_count 开始的新消息切片
    pub fn get_unpersisted_messages(&self) -> &[ChatMessage] {
        &self.messages[self.persisted_count..]
    }

    /// 标记当前所有消息为已持久化
    pub fn mark_persisted(&mut self) {
        self.persisted_count = self.messages.len();
    }

    /// 构建系统提示词
    pub fn build_system_prompt(workspace_path: &str) -> String {
        let design_guides = get_all_design_guides();
        format!(
            "你是 DocAgent，一个专业的 AI 文档处理助手。\n\
            \n\
            你可以使用两类工具：\n\
            \n\
            **Tools（基础工具，始终可用）：**\n\
            - list_directory: 列出目录内容\n\
            - search_files: 搜索文件（按名称/内容/扩展名）\n\
            - read_file: 读取纯文本文件（.txt/.md/.csv/.json 等）\n\
            - file_info: 获取文件元数据（大小、修改时间、类型）\n\
            - file_exists: 检查文件或目录是否存在\n\
            - delete_file: 删除文件（高风险，需用户确认）\n\
            - create_directory: 创建目录\n\
            - write_text_file: 写入纯文本文件\n\
            \n\
            **Skills（高级技能，依赖文档处理引擎）：**\n\
            - generate_document: 生成结构化文档（Word/Excel/PPT/PDF/Markdown）\n\
            - read_document: 读取结构化文档内容（Word/Excel/PPT/PDF）\n\
            - modify_document: 修改已有文档\n\
            - convert_format: 转换文档格式\n\
            - analyze_document: 分析文档结构和统计信息\n\
            - batch_process: 批量处理多个文档\n\
            \n\
            使用建议：\n\
            - 读取纯文本文件时，优先使用 read_file（更快，不依赖 Sidecar）\n\
            - 读取 Word/Excel/PPT/PDF 等结构化文档时，使用 read_document\n\
            - 只需查看文件信息时，使用 file_info 而非读取整个文件\n\
            \n\
            当前工作区路径: {}\n\
            \n\
            工作原则：\n\
            1. 在执行任何修改操作前，先确认用户的意图\n\
            2. 对于重要操作（如删除、覆盖），需要明确提醒用户\n\
            3. 优先使用工具完成任务，而不是仅提供建议\n\
            4. 如果操作可能造成数据丢失，先创建版本快照\n\
            5. 使用中文与用户交流\n\
            \n\
            ---\n\
            以下是你生成文档时必须遵循的专业设计规范：\n\
            \n\
            {}",
            workspace_path,
            design_guides
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：生成指定字符数的字符串
    fn make_long_string(char_count: usize) -> String {
        "a".repeat(char_count)
    }

    /// 测试第一轮迭代时系统提示词不追加继续推理提示
    #[test]
    fn test_get_messages_for_iteration_first_iteration() {
        let mut ctx = AgentContext::new("session-1".to_string(), "你是助手".to_string());
        ctx.add_user_message("你好");
        ctx.add_assistant_message("你好！", None, None);

        let messages = ctx.get_messages_for_iteration(1);

        // 系统消息应该是原始系统提示词，不包含继续推理提示
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "你是助手");
        assert!(!messages[0].content.contains("继续执行之前的任务"));
    }

    /// 测试后续迭代时系统提示词追加继续推理提示
    #[test]
    fn test_get_messages_for_iteration_later_iteration() {
        let mut ctx = AgentContext::new("session-1".to_string(), "你是助手".to_string());
        ctx.add_user_message("你好");
        ctx.add_assistant_message("你好！", None, None);

        let messages = ctx.get_messages_for_iteration(2);

        // 系统消息应该包含继续推理提示
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("继续执行之前的任务"));
        assert!(messages[0].content.starts_with("你是助手"));
    }

    /// 测试早期 reasoning_content 超过阈值时被压缩
    #[test]
    fn test_get_messages_for_iteration_compress_reasoning() {
        let mut ctx = AgentContext::new("session-1".to_string(), "你是助手".to_string());
        ctx.add_user_message("你好");

        // 早期的 assistant 消息，reasoning_content 超过阈值（600 > 500）
        let long_reasoning = make_long_string(600);
        ctx.add_assistant_message("回复1", None, Some(long_reasoning));

        // 最近一条有 reasoning_content 的消息（使其成为"最近一轮"）
        ctx.add_user_message("继续");
        ctx.add_assistant_message("回复2", None, Some("短推理".to_string()));

        let messages = ctx.get_messages_for_iteration(1);

        // 消息布局: [system, user, assistant(早期), user, assistant(最近)]
        let early_assistant = &messages[2];

        // 早期 reasoning_content 应该被压缩，包含省略标记
        let compressed = early_assistant.reasoning_content.as_ref().unwrap();
        assert!(compressed.contains("...(已省略)"));

        // 压缩后应该以原始内容的前 200 字符开头
        let expected_prefix = make_long_string(REASONING_COMPRESS_KEEP);
        assert!(compressed.starts_with(&expected_prefix));
    }

    /// 测试最近一轮的 reasoning_content 保持完整
    #[test]
    fn test_get_messages_for_iteration_keep_latest_reasoning() {
        let mut ctx = AgentContext::new("session-1".to_string(), "你是助手".to_string());
        ctx.add_user_message("你好");

        // 早期长 reasoning
        let long_reasoning = make_long_string(600);
        ctx.add_assistant_message("回复1", None, Some(long_reasoning));

        // 最近一条长 reasoning（超过阈值但不应被压缩，因为是最新的）
        let latest_reasoning = make_long_string(700);
        ctx.add_user_message("继续");
        ctx.add_assistant_message("回复2", None, Some(latest_reasoning.clone()));

        let messages = ctx.get_messages_for_iteration(1);

        // 消息布局: [system, user, assistant(早期), user, assistant(最近)]
        let latest_assistant = &messages[4];

        // 最近一条 assistant 消息的 reasoning_content 应该保持完整
        assert_eq!(latest_assistant.reasoning_content.as_ref().unwrap(), &latest_reasoning);
        assert!(!latest_assistant.reasoning_content.as_ref().unwrap().contains("...(已省略)"));
    }

    /// 测试短 reasoning_content 不被压缩
    #[test]
    fn test_get_messages_for_iteration_short_reasoning_not_compressed() {
        let mut ctx = AgentContext::new("session-1".to_string(), "你是助手".to_string());
        ctx.add_user_message("你好");

        // 短 reasoning（不超过阈值 500）
        let short_reasoning = "这是一个简短的推理过程".to_string();
        ctx.add_assistant_message("回复1", None, Some(short_reasoning.clone()));

        // 最近一条也有 reasoning，使早期的成为"非最新"
        ctx.add_user_message("继续");
        ctx.add_assistant_message("回复2", None, Some("最新推理".to_string()));

        let messages = ctx.get_messages_for_iteration(1);

        // 消息布局: [system, user, assistant(早期), user, assistant(最近)]
        let early_assistant = &messages[2];

        // 短 reasoning 不应该被压缩
        assert_eq!(early_assistant.reasoning_content.as_ref().unwrap(), &short_reasoning);
        assert!(!early_assistant.reasoning_content.as_ref().unwrap().contains("...(已省略)"));
    }
}
