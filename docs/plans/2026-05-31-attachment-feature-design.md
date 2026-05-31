# 附加文件功能开发计划

**目标**: 在输入框左侧实现附加文件按钮，支持用户上传图片和文档文件，将附件内容正确传递给大模型处理。

**架构**: 前端负责文件选择、预览和状态管理；Rust 后端负责文件读取、文档解析和图片编码；LLM 适配器层负责将多模态内容转换为各 Provider 的 API 格式。

**技术栈**: Tauri Dialog API (文件选择) + Rust base64 编码 + Python Sidecar (文档解析) + LLM Vision API (图片多模态)

---

## 一、现状分析

### 1.1 当前代码状态

| 模块 | 现状 | 需要改动 |
|------|------|----------|
| InputArea 组件 | 附加按钮已存在但无 onClick 处理 | 添加文件选择逻辑和附件预览 |
| ChatMessage 模型 | `content: String` 纯文本 | 扩展为支持多模态内容数组 |
| LLM 适配器 | 仅发送纯文本 content | 各适配器需支持图片/多模态格式 |
| start_agent 命令 | 仅接收 `prompt: String` | 需接收附件信息 |
| AgentContext | `add_user_message` 仅接受文本 | 需支持带附件的用户消息 |
| 数据库 session_messages | `content TEXT` 纯文本 | 需新增 attachments 列 |
| ProviderConfig | 无视觉能力标记 | 需新增 `supports_vision` 字段 |

### 1.2 各 LLM Provider 图片 API 格式差异

**OpenAI (GPT-4o / GPT-4V)**:
```json
{
  "role": "user",
  "content": [
    {"type": "text", "text": "描述这张图片"},
    {"type": "image_url", "image_url": {"url": "data:image/png;base64,...", "detail": "auto"}}
  ]
}
```

**Anthropic (Claude 3.5 Sonnet)**:
```json
{
  "role": "user",
  "content": [
    {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "..."}},
    {"type": "text", "text": "描述这张图片"}
  ]
}
```

**Gemini (Gemini 1.5 Pro)**:
```json
{
  "role": "user",
  "parts": [
    {"inline_data": {"mime_type": "image/png", "data": "..."}},
    {"text": "描述这张图片"}
  ]
}
```

### 1.3 各 Provider 视觉能力

| Provider 类型 | 支持视觉的模型示例 | 不支持视觉的模型示例 |
|--------------|-------------------|---------------------|
| OpenAI | gpt-4o, gpt-4-vision-preview, gpt-4-turbo | gpt-3.5-turbo, text-davinci |
| Anthropic | claude-3-5-sonnet, claude-3-opus | claude-3-haiku (有限支持) |
| Gemini | gemini-1.5-pro, gemini-1.5-flash | gemini-1.0-pro |
| Ollama | llava, bakllava, moondream | llama3, mistral, codellama |

---

## 二、核心设计决策

### 2.1 附件类型分类

将附件分为两大类，采用不同的处理策略：

**图片类** (image/png, image/jpeg, image/gif, image/webp):
- 支持视觉的模型: 以原生多模态格式发送 (base64 编码)
- 不支持视觉的模型: 在文本中描述 "[用户上传了图片: filename.png (1280x720, 256KB)]"，由 LLM 根据文件名推断上下文

**文档/文件类** (.docx, .xlsx, .pptx, .pdf, .txt, .md, .csv, .json, .html 等):
- 解析文档内容为纯文本，注入到用户消息中
- 大文档截断策略: 超过 Token 预算限制时，截断并添加省略提示

### 2.2 附件数据流

```
用户选择文件
    |
    v
前端: 文件选择器 -> 读取文件元信息 (名称/大小/类型)
    |
    v
前端: 在输入框上方显示附件预览条 (缩略图/文件图标+名称+大小)
    |
    v
用户点击发送
    |
    v
前端: 调用 start_agent(sessionId, prompt, { attachments: [...] })
    |
    v
Rust 后端: start_agent 接收 attachments 参数
    |
    v
Rust 后端: 遍历附件列表
    |- 图片: 读取文件 -> base64 编码 -> 存入 AttachmentContent::Image
    |- 文档: 调用 Sidecar read_document -> 提取文本 -> 存入 AttachmentContent::Document
    |- 纯文本: 直接读取文件内容 -> 存入 AttachmentContent::Text
    |
    v
Rust 后端: 构建多模态 ChatMessage (content 为 ContentPart 数组)
    |
    v
Rust 后端: 各 LLM 适配器将 ContentPart 数组转换为对应 API 格式
    |
    v
LLM API 请求
```

### 2.3 Provider 视觉能力检测方案

采用**配置 + 模型名称推断**的双重策略：

1. **ProviderConfig 新增 `supports_vision` 可选字段**: 用户可在设置中手动标记模型是否支持视觉
2. **模型名称自动推断**: 当 `supports_vision` 未配置时，根据模型名称关键词自动推断
   - 包含 `gpt-4o`, `gpt-4-vision`, `gpt-4-turbo`, `vision` -> 支持
   - 包含 `claude-3`, `claude-3.5` -> 支持
   - 包含 `gemini-1.5`, `gemini-pro-vision` -> 支持
   - 包含 `llava`, `bakllava`, `moondream` -> 支持
   - 其他 -> 不支持

### 2.4 文档解析策略

| 文件类型 | 解析方式 | 说明 |
|---------|---------|------|
| .txt, .md, .csv, .json, .html, .xml, .log, .yaml, .toml | Rust 端直接读取 | 纯文本文件，1MB 上限 |
| .docx | Sidecar read_document | 提取段落文本和表格 |
| .xlsx | Sidecar read_document | 提取单元格数据 |
| .pptx | Sidecar read_document | 提取幻灯片文本 |
| .pdf | Sidecar read_document | 提取文本内容 |
| 其他二进制格式 | 拒绝上传 | 提示用户不支持该格式 |

### 2.5 附件大小限制

- 单张图片: 最大 20MB (OpenAI 限制)
- 单个文档: 最大 10MB
- 单次发送附件总数: 最多 10 个
- 文档解析后文本: 最大 100,000 字符 (约 25K tokens)，超出截断

---

## 三、数据模型设计

### 3.1 前端类型定义

```typescript
// 附件类型枚举
type AttachmentType = "image" | "document" | "text";

// 附件元信息 (前端 -> 后端)
interface AttachmentMeta {
  // 文件在工作区中的相对路径 (工作区内文件)
  path?: string;
  // 文件绝对路径 (工作区外文件)
  absolutePath?: string;
  // 文件名
  name: string;
  // MIME 类型
  mimeType: string;
  // 文件大小 (字节)
  size: number;
  // 附件类型
  type: AttachmentType;
}

// 附件预览信息 (前端状态)
interface AttachmentPreview {
  id: string;
  name: string;
  size: number;
  type: AttachmentType;
  mimeType: string;
  // 图片缩略图 (仅图片类型，base64 data URL)
  thumbnail?: string;
  // 加载状态
  status: "loading" | "ready" | "error";
  errorMessage?: string;
}
```

### 3.2 Rust 数据模型

```rust
/// 附件元信息 (从前端接收)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    pub path: Option<String>,
    pub absolute_path: Option<String>,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub attachment_type: AttachmentType,
}

/// 附件类型
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentType {
    Image,
    Document,
    Text,
}

/// 解析后的附件内容
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AttachmentContent {
    /// 图片: base64 编码数据
    Image {
        mime_type: String,
        data: String, // base64
    },
    /// 文档: 解析后的文本内容
    Document {
        text: String,
        source_format: String, // "docx", "xlsx", "pptx", "pdf"
    },
    /// 纯文本: 直接读取的文件内容
    Text {
        content: String,
    },
}

/// 多模态内容部分 (ChatMessage 的 content 扩展)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// 文本部分
    Text {
        text: String,
    },
    /// 图片部分 (base64)
    Image {
        mime_type: String,
        data: String,
    },
}
```

### 3.3 ChatMessage 模型扩展

当前 `ChatMessage.content` 是 `String` 类型。为了兼容现有逻辑并支持多模态，采用**双字段方案**：

```rust
pub struct ChatMessage {
    pub role: String,
    /// 纯文本内容 (向后兼容，纯文本消息时使用)
    pub content: String,
    /// 多模态内容部分 (有附件时使用，content 为空或纯文本摘要)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_parts: Option<Vec<ContentPart>>,
    pub tool_calls: Option<Vec<LlmToolCall>>,
    pub tool_call_id: Option<String>,
    pub reasoning_content: Option<String>,
}
```

**规则**:
- 无附件时: `content` 为文本，`content_parts` 为 `None`
- 有附件时: `content_parts` 包含所有内容部分 (文本 + 图片/文档)，`content` 为纯文本摘要 (用于 Token 估算和历史显示)
- LLM 适配器优先使用 `content_parts`，回退到 `content`

### 3.4 数据库扩展

在 `session_messages` 表新增 `attachments` 列:

```sql
ALTER TABLE session_messages ADD COLUMN attachments TEXT DEFAULT NULL;
```

`attachments` 存储序列化后的 JSON 数组，格式:
```json
[
  {"type": "image", "name": "screenshot.png", "mimeType": "image/png", "size": 256000},
  {"type": "document", "name": "report.docx", "mimeType": "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "size": 1024000}
]
```

**注意**: 不存储 base64 图片数据和文档解析文本 (体积过大)，仅存储元信息。历史消息恢复时，附件内容以文本摘要形式存在于 `content` 字段中。

### 3.5 ProviderConfig 扩展

```rust
pub struct ProviderConfig {
    // ... 现有字段 ...
    /// 是否支持视觉/图片多模态 (None 表示自动推断)
    #[serde(default)]
    pub supports_vision: Option<bool>,
}
```

---

## 四、各 LLM 适配器多模态支持

### 4.1 OpenAI 适配器

```rust
// 将 ContentPart 数组转换为 OpenAI 格式
fn convert_content_parts(parts: &[ContentPart]) -> Value {
    json!(parts.iter().map(|part| match part {
        ContentPart::Text { text } => json!({
            "type": "text",
            "text": text,
        }),
        ContentPart::Image { mime_type, data } => json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{};base64,{}", mime_type, data),
                "detail": "auto",
            }
        }),
    }).collect::<Vec<_>>())
}
```

### 4.2 Anthropic 适配器

```rust
// 将 ContentPart 数组转换为 Anthropic 格式
fn convert_content_parts(parts: &[ContentPart]) -> Vec<Value> {
    parts.iter().map(|part| match part {
        ContentPart::Text { text } => json!({
            "type": "text",
            "text": text,
        }),
        ContentPart::Image { mime_type, data } => json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": mime_type,
                "data": data,
            }
        }),
    }).collect::<Vec<_>>()
}
```

### 4.3 Gemini 适配器

```rust
// 将 ContentPart 数组转换为 Gemini parts 格式
fn convert_content_parts(parts: &[ContentPart]) -> Vec<Value> {
    parts.iter().map(|part| match part {
        ContentPart::Text { text } => json!({"text": text}),
        ContentPart::Image { mime_type, data } => json!({
            "inline_data": {
                "mime_type": mime_type,
                "data": data,
            }
        }),
    }).collect::<Vec<_>>()
}
```

### 4.4 不支持视觉的 Provider 降级策略（防止幻觉）

**核心问题**: 如果仅在文本中描述 `[用户上传了图片: screenshot.png]`，LLM 可能会"幻想"图片内容，编造图片中有什么，这是严重的数据诚实性问题。

**解决方案**: 采用"明确不可见 + 系统约束 + 替代建议"三重防护。

#### 4.4.1 用户消息中的图片描述格式

当检测到当前 Provider 不支持视觉时，图片以以下格式注入用户消息:

```
[用户上传了图片: {filename} ({width}x{height}, {size_kb}KB)]

<image_visibility_warning>
重要提示：当前模型不支持图片识别功能，无法看到这张图片的实际内容。
请勿猜测或幻想图片中的内容。如果需要处理图片，请：
1. 切换到支持视觉的模型（如 GPT-4o、Claude 3.5 Sonnet、Gemini 1.5 Pro）
2. 或者由用户在文字中描述图片内容
</image_visibility_warning>
```

#### 4.4.2 系统提示词注入

在 Agent 执行时，如果检测到用户消息包含图片但当前模型不支持视觉，动态追加系统提示词片段:

```rust
// 在 AgentContext::get_messages_for_iteration 中
if has_image_attachments && !supports_vision {
    let vision_warning = r#"
<vision_constraint>
当前使用的模型不支持图片识别（视觉）功能。
用户消息中可能包含图片附件，但你无法看到这些图片的实际内容。
你必须遵守以下规则：
1. 绝对不要猜测、幻想或编造图片中的内容
2. 如果用户请求需要分析图片内容，明确告知用户你无法看到图片
3. 建议用户切换到支持视觉的模型，或在文字中描述图片内容
</vision_constraint>
"#;
    // 追加到系统提示词末尾
}
```

#### 4.4.3 前端用户提示

在发送前检测，如果附件包含图片但当前模型不支持视觉:

1. **发送前确认弹窗**:
   ```
   当前模型 "{model_name}" 不支持图片识别。
   
   图片将以"不可见"状态发送，AI 将无法看到图片内容。
   
   [ ] 不再提示
   [继续发送]  [切换模型]
   ```

2. **附件预览条标记**: 图片附件显示警告图标，hover 提示 "当前模型无法识别此图片"

#### 4.4.4 完整降级流程

```
用户添加图片附件
    |
    v
前端检测当前 Provider 是否支持视觉
    |
    +-- 支持 --> 正常发送图片 (base64 多模态)
    |
    +-- 不支持 --> 显示警告提示
                        |
                        v
                  用户选择:
                  [继续发送] --> 图片以"不可见"格式注入文本
                                    |
                                    v
                              后端在系统提示词中注入 vision_constraint
                                    |
                                    v
                              LLM 收到明确的"不可见"约束
                  [切换模型] --> 打开设置页面
```

#### 4.4.5 为什么这样设计能防止幻觉

| 防护层 | 作用 |
|-------|------|
| 用户消息中的 `<image_visibility_warning>` | 明确告知 LLM 图片不可见，提供替代方案 |
| 系统提示词中的 `<vision_constraint>` | 以系统指令形式约束 LLM 行为，禁止幻想 |
| 前端发送前确认 | 让用户知情，避免无效交互 |
| LLM 自身的安全训练 | 现代模型对"不可见"和"不要幻想"指令有较好的遵循能力 |

**关键原则**: 宁可明确告知"无法看到"，绝不给 LLM 留下幻想空间。

---

## 五、前端设计

### 5.1 附件选择交互

**触发方式**:
1. 点击附加文件按钮 -> 弹出文件选择器 (Tauri Dialog API)
2. 拖拽文件到输入框区域
3. 粘贴剪贴板图片 (Ctrl+V)

**文件选择器配置**:
```
图片模式: 过滤 .png, .jpg, .jpeg, .gif, .webp
文档模式: 过滤 .docx, .xlsx, .pptx, .pdf, .txt, .md, .csv, .json
全部模式: 合并以上所有格式
```

### 5.2 附件预览条

在输入框上方显示已添加的附件列表:

```
+------------------------------------------------------------------+
| [x] screenshot.png  1280x720  256KB    [x] report.docx  1.2MB   |
| [缩略图]                                     [docx图标]          |
+------------------------------------------------------------------+
| [附加] [输入框................................] [模板] [发送]     |
+------------------------------------------------------------------+
```

- 图片附件: 显示缩略图 + 文件名 + 大小
- 文档附件: 显示文件类型图标 + 文件名 + 大小
- 每个附件右侧有删除按钮 [x]
- 附件数量达到上限时禁用附加按钮

### 5.3 InputArea 组件改动

```typescript
interface InputAreaProps {
  onSend: (text: string, attachments?: AttachmentMeta[]) => void;  // 扩展签名
  disabled?: boolean;
  executionStatus?: ExecutionStatus;
  onStop?: () => void;
  workspacePath?: string;  // 用于解析工作区内文件路径
}
```

### 5.4 useAgent Hook 改动

```typescript
// sendMessage 扩展
sendMessage: (prompt: string, options?: {
  attachments?: AttachmentMeta[];
  maxIterations?: number;
  workingDirectory?: string;
  workspaceId?: string;
}) => Promise<void>;
```

### 5.5 前端状态管理

在 `useWorkflowStore` 或新建 `useAttachmentStore` 中管理:
- 当前输入框的附件列表
- 附件预览数据 (缩略图)
- 附件加载状态

---

## 六、后端改动

### 6.1 新增 Tauri 命令

```rust
/// 解析附件内容 (图片 base64 编码 / 文档文本提取)
#[tauri::command]
pub async fn resolve_attachment(
    attachment: AttachmentMeta,
    workspace_root: String,
    state: State<'_, AppState>,
) -> Result<ResolvedAttachment, CommandError>
```

此命令可选 -- 也可以在 `start_agent` 内部直接处理附件解析，避免额外的前后端通信。

### 6.2 start_agent 命令扩展

```rust
#[tauri::command]
pub async fn start_agent(
    session_id: String,
    prompt: String,
    options: Option<serde_json::Value>,  // 新增 attachments 字段
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError>
```

`options` 中新增:
```json
{
  "attachments": [
    {
      "path": "images/screenshot.png",
      "name": "screenshot.png",
      "mimeType": "image/png",
      "size": 256000,
      "type": "image"
    }
  ]
}
```

### 6.3 附件解析服务

新增 `src-tauri/src/services/attachment.rs`:

```rust
pub struct AttachmentService;

impl AttachmentService {
    /// 解析附件列表，返回 ContentPart 数组
    pub async fn resolve_attachments(
        attachments: &[AttachmentMeta],
        workspace_root: &str,
        doc_service: &DocumentService,
        supports_vision: bool,
    ) -> Result<Vec<ContentPart>, CommandError> {
        let mut parts = Vec::new();
        for attachment in attachments {
            match attachment.attachment_type {
                AttachmentType::Image => {
                    let (data, mime_type) = Self::read_image(attachment, workspace_root)?;
                    if supports_vision {
                        parts.push(ContentPart::Image { mime_type, data });
                    } else {
                        // 降级: 仅在文本中提及
                        parts.push(ContentPart::Text {
                            text: format!("[用户上传了图片: {} ({}KB)]",
                                attachment.name,
                                attachment.size / 1024
                            ),
                        });
                    }
                }
                AttachmentType::Document => {
                    let text = Self::read_document(attachment, workspace_root, doc_service).await?;
                    parts.push(ContentPart::Text {
                        text: format!("[用户上传了文档: {}]\n{}\n[/文档结束]",
                            attachment.name, text),
                    });
                }
                AttachmentType::Text => {
                    let content = Self::read_text_file(attachment, workspace_root)?;
                    parts.push(ContentPart::Text {
                        text: format!("[用户上传了文件: {}]\n{}\n[/文件结束]",
                            attachment.name, content),
                    });
                }
            }
        }
        Ok(parts)
    }
}
```

### 6.4 AgentContext 扩展

```rust
impl AgentContext {
    /// 添加带附件的用户消息
    pub fn add_user_message_with_attachments(
        &mut self,
        content: &str,
        content_parts: Option<Vec<ContentPart>>,
    ) {
        // ... 任务类型识别逻辑 ...
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
            content_parts,
            tool_calls: None,
            tool_call_id: None,
            reasoning_content: None,
        });
    }
}
```

### 6.5 数据库迁移

新增迁移脚本，为 `session_messages` 表添加 `attachments` 列:

```sql
-- 迁移 V2: 添加附件支持
ALTER TABLE session_messages ADD COLUMN attachments TEXT DEFAULT NULL;
```

---

## 七、任务分解

### 阶段一: 基础架构 (数据模型 + 后端核心)

**任务 1.1: 扩展 Rust 数据模型**
- 文件: `src-tauri/src/models/llm.rs`
- 新增 `ContentPart` 枚举 (Text, Image)
- 扩展 `ChatMessage` 添加 `content_parts` 字段
- 文件: `src-tauri/src/models/message.rs`
- 新增 `AttachmentMeta` 结构体
- 扩展 `Message` 模型添加 `attachments` 字段
- 更新 `to_chat_message` 方法处理 `content_parts`

**任务 1.2: 扩展 ProviderConfig 支持视觉标记**
- 文件: `src-tauri/src/config/llm_config.rs`
- `ProviderConfig` 新增 `supports_vision: Option<bool>` 字段
- 新增 `resolve_supports_vision()` 方法 (配置优先 + 模型名推断)
- 文件: `src-tauri/src/models/llm.rs`
- `ProviderInfo` 新增 `supports_vision: bool` 字段

**任务 1.3: 数据库迁移**
- 文件: `src-tauri/src/db/init.rs`
- 新增 `attachments` 列迁移
- 文件: `src-tauri/src/db/message_repo.rs`
- `create_message` 新增 `attachments` 参数
- 新增 `list_messages_with_attachments` 查询方法

**任务 1.4: 附件解析服务**
- 新建: `src-tauri/src/services/attachment.rs`
- 实现 `AttachmentService::resolve_attachments`
- 图片读取 + base64 编码
- 文档解析 (调用 Sidecar)
- 纯文本文件读取
- 视觉能力降级处理
- 在 `mod.rs` 中注册模块

**任务 1.5: 扩展 AgentContext 和 start_agent**
- 文件: `src-tauri/src/services/agent/context.rs`
- 新增 `add_user_message_with_attachments` 方法
- 新增 `has_image_attachments` 标记
- `get_messages_for_iteration` 中检测图片 + 不支持视觉时注入 `<vision_constraint>` 系统提示词片段
- 文件: `src-tauri/src/commands/agent.rs`
- `start_agent` 从 options 提取 attachments
- 调用 AttachmentService 解析附件
- 构建带 content_parts 的用户消息
- 持久化时保存 attachments 元信息
- 传递 `supports_vision` 标记给 AgentContext

**任务 1.6: 附件解析服务中的幻觉防护**
- 文件: `src-tauri/src/services/attachment.rs`
- 图片降级时注入 `<image_visibility_warning>` 标签
- 包含明确的"不可见"提示和替代建议

### 阶段二: LLM 适配器多模态支持

**任务 2.1: OpenAI 适配器多模态**
- 文件: `src-tauri/src/services/llm/openai_adapter.rs`
- `build_request_body` 中: 当 `content_parts` 存在时，构建 content 数组格式
- 图片 -> `image_url` 类型 content part
- 纯文本 -> `text` 类型 content part

**任务 2.2: Anthropic 适配器多模态**
- 文件: `src-tauri/src/services/llm/anthropic_adapter.rs`
- `convert_messages` 中: 当 `content_parts` 存在时，构建 content blocks 数组
- 图片 -> `image` 类型 content block
- 纯文本 -> `text` 类型 content block

**任务 2.3: Gemini 适配器多模态**
- 文件: `src-tauri/src/services/llm/gemini_adapter.rs`
- `build_request_body` 中: 当 `content_parts` 存在时，构建 parts 数组
- 图片 -> `inline_data` part
- 纯文本 -> `text` part

### 阶段三: 前端实现

**任务 3.1: 前端类型定义**
- 文件: `src/types/` 或 `shared/types.ts`
- 新增 `AttachmentMeta`, `AttachmentPreview`, `AttachmentType` 类型
- 扩展前端消息类型添加 `attachments` 和 `contentParts` 字段

**任务 3.2: 附件状态管理**
- 新建或扩展: `src/stores/attachmentStore.ts`
- 管理当前输入框的附件列表
- 附件添加/删除/清空操作
- 附件预览数据 (缩略图)

**任务 3.3: InputArea 组件改造**
- 文件: `src/components/layout/InputArea.tsx`
- 附加按钮绑定文件选择器 (Tauri Dialog API)
- 附件预览条 UI (输入框上方)
- 拖拽文件支持
- 剪贴板粘贴图片支持
- `onSend` 签名扩展 (传递 attachments)

**任务 3.4: 视觉不支持时的前端提示和确认**
- 新建或复用: `src/components/common/ConfirmDialog.tsx` 或类似组件
- 发送前检测: 附件包含图片 + 当前 Provider 不支持视觉
- 显示确认弹窗: "当前模型不支持图片识别，图片将以不可见状态发送"
- 选项: [继续发送] [切换模型]
- 附件预览条中图片显示警告图标 (当模型不支持视觉时)
- 从 settingsStore 或 providerStore 获取当前 Provider 的 `supports_vision` 状态

**任务 3.5: useAgent Hook 扩展**
- 文件: `src/hooks/useAgent.ts`
- `sendMessage` 支持 attachments 参数
- 将附件信息传入 `startAgent` 的 options

**任务 3.6: tauri.ts 服务层扩展**
- 文件: `src/services/tauri.ts`
- `startAgent` 函数支持传递 attachments

**任务 3.7: 工作流节点展示**
- 文件: `src/components/workflow/UserNode.tsx` (或相关组件)
- 用户消息节点显示附件信息 (图片缩略图/文件名)
- 如果图片在不支持视觉的模型下发送，显示"不可见"标记

### 阶段四: 设置界面支持

**任务 4.1: Provider 配置视觉标记**
- 文件: `src/components/settings/LLMConfig.tsx` 或 `ProviderFormDialog.tsx`
- 新增 "支持视觉/图片" 开关
- 保存到 ProviderConfig 的 `supports_vision` 字段

---

## 八、边界情况与错误处理

### 8.1 图片相关

| 场景 | 处理方式 |
|------|---------|
| 图片超过 20MB | 前端拒绝添加，提示 "图片不能超过 20MB" |
| 图片格式不支持 (如 BMP, TIFF) | 前端拒绝添加，提示 "不支持的图片格式" |
| 当前模型不支持视觉 | 前端显示确认弹窗告知用户；后端注入 `<image_visibility_warning>` 和 `<vision_constraint>` 防止幻觉 |
| 图片 base64 编码失败 | 返回错误，提示 "图片处理失败" |
| 图片损坏无法读取 | 返回错误，提示 "图片文件损坏" |
| LLM 仍然幻想图片内容 | 系统提示词约束 + 用户消息明确告知不可见，双重防护；若仍幻想属于模型行为问题，建议用户切换模型 |

### 8.2 文档相关

| 场景 | 处理方式 |
|------|---------|
| 文档超过 10MB | 前端拒绝添加，提示 "文档不能超过 10MB" |
| 文档格式不支持 | 前端拒绝添加，提示 "不支持的文件格式" |
| Sidecar 解析超时 | 返回错误，提示 "文档解析超时" |
| 文档解析后文本过长 | 截断至 100,000 字符，添加 "[内容过长已截断]" 标记 |
| 文档为空或无文本内容 | 注入 "[文档 {name} 为空或无可提取文本]" |
| 文档受密码保护 | 返回错误，提示 "文档受密码保护，无法解析" |
| Sidecar 进程崩溃 | 自动重启 Sidecar 并重试一次 |

### 8.3 通用

| 场景 | 处理方式 |
|------|---------|
| 附件数量超过 10 个 | 前端拒绝添加，提示 "最多添加 10 个附件" |
| 同时发送图片和文档 | 图片走多模态路径，文档走文本注入路径，互不影响 |
| 发送时附件正在解析 | 等待解析完成后再发送，或显示加载状态 |
| 附件文件不存在 | 返回错误，提示 "文件不存在" |
| 附件文件在工作区外 | 通过 absolute_path 读取，但需安全校验 |
| 会话历史恢复 | 附件元信息从数据库恢复，但 base64 数据不恢复 (以文本摘要代替) |
| Agent 迭代中引用附件 | content_parts 在历史消息中完整保留，供后续迭代使用 |

### 8.4 Token 预算

| 场景 | 处理方式 |
|------|---------|
| 附件解析后文本超出 Token 预算 | 按优先级截断: 文档文本优先截断，图片描述保留 |
| 图片 base64 占用大量 Token | 图片 Token 由 LLM API 自动计算，需在上下文窗口使用信息中反映 |
| 多个文档总文本过长 | 逐个截断，每个文档保留前 N 字符 |

---

## 九、安全考虑

1. **路径遍历防护**: 附件路径必须经过与 Tool 系统相同的 `workspace_root` 安全校验，拒绝 `../` 等路径遍历攻击
2. **文件大小限制**: 前端和后端双重校验，防止恶意大文件消耗内存
3. **MIME 类型校验**: 不信任客户端提供的 MIME 类型，后端根据文件扩展名和魔数二次校验
4. **Base64 内存安全**: 大图片的 base64 编码可能占用大量内存，需限制并发处理数量
5. **Sidecar 调用安全**: 文档解析通过现有 Sidecar 协议，继承其超时和重试机制

---

## 十、实现优先级

1. **P0 (核心)**: 图片上传 + 多模态 API 支持 + **幻觉防护机制** (阶段一 + 阶段二 + 阶段三核心)
   - 幻觉防护是数据诚实的关键，必须在 P0 阶段实现
   - 包括: `<image_visibility_warning>` 注入、`<vision_constraint>` 系统提示词、前端确认弹窗
2. **P1 (重要)**: 文档上传 + 文本解析注入
3. **P2 (增强)**: 拖拽上传、剪贴板粘贴、视觉能力设置 UI
4. **P3 (优化)**: 附件预览优化、Token 预算精细控制、大文件流式处理

建议先实现 P0 核心功能（含幻觉防护），验证端到端流程后再逐步添加 P1-P3 功能。
