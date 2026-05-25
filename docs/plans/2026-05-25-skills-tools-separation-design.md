# Skills 与 Tools 分离重构开发计划

## 一、现状分析

### 1.1 当前架构：Skills 与 Tools 混为一体

当前 DocAgent 项目中，**Skills 即 Tools，Tools 即 Skills**，二者没有概念上的区分：

- 所有能力统一实现为 `Skill` trait（[registry.rs](../src-tauri/src/services/skill/registry.rs)）
- `SkillRegistry.tool_definitions()` 将所有 Skill 转换为 OpenAI Function Calling 格式的工具定义
- `AgentExecutor` 将工具定义传给 LLM，LLM 返回 `tool_calls` 后，executor 按 `tool_call.name` 从 `SkillRegistry` 查找并执行
- 三个 LLM 适配器（OpenAI / Anthropic / Gemini）均已完整支持 Tool Calling，包括流式增量合并

**关键代码路径**：

```
SkillRegistry.tool_definitions()
  → AgentExecutor.execute() 中转为 Vec<ToolDefinition>
    → LlmRouter.chat_stream(&messages, &tools)
      → LLM 返回 tool_calls
        → AgentExecutor 按 name 查找 SkillRegistry 中的 Skill 执行
```

### 1.2 当前 9 个内置 Skill 清单

| Skill 名称 | 实现方式 | 依赖 Sidecar | 风险等级 | 调用频率 | 复杂度 |
|-------------|----------|-------------|---------|---------|--------|
| `generate_document` | Sidecar (Python) | 是 | 高 | 中 | 高 |
| `read_document` | Sidecar (Python) | 是 | 低 | **极高** | 中 |
| `modify_document` | Sidecar (Python) | 是 | 高 | 中 | 高 |
| `delete_document` | Rust 原生 | 否 | **极高** | 低 | 低 |
| `convert_format` | Sidecar (Python) | 是 | 中 | 低 | 高 |
| `search_documents` | Rust 原生 | 否 | 低 | **极高** | 低 |
| `analyze_document` | Sidecar (Python) | 是 | 低 | 中 | 中 |
| `list_workspace` | Rust 原生 | 否 | 低 | **极高** | 低 |
| `batch_process` | Sidecar (Python) | 是 | 高 | 低 | 高 |

### 1.3 核心问题

1. **概念混淆**：简单的文件系统操作（列出目录、搜索文件、删除文件）和复杂的文档处理（生成 Word、转换格式）被放在同一层级，用户无法区分
2. **无法独立控制**：用户只能全局启用/禁用 Skill，但像"列出目录"这种基础能力不应该被禁用
3. **LLM 上下文浪费**：所有 9 个 Skill 的定义都发送给 LLM，但简单操作（如列出目录）的参数定义与复杂操作（如批量处理）混在一起，增加了 LLM 的选择负担
4. **扩展性差**：如果要添加"检查文件是否存在"、"获取文件信息"等简单操作，按当前架构必须创建完整的 Skill，过于重量级
5. **前端展示不合理**：设置页面将所有能力统一展示为"Skills"，用户无法区分基础工具和高级技能

### 1.4 已有基础设施

项目已具备完整的 Tool Calling 基础设施，无需从零搭建：

- **LLM 模型层**：`ToolDefinition`、`LlmToolCall`、`ChatMessage.tool_calls`、`ChatMessage.tool_call_id` 已定义
- **LLM 适配器**：三个适配器均支持 `tools` 参数传递和 `tool_calls` 响应解析
- **Agent 执行器**：完整的 Tool Calling 循环（收集增量 → 合并 → 查找执行 → 返回结果）
- **前端工作流**：`ToolNodeData` 类型、`ToolNode` 组件已实现
- **事件系统**：`agent:tool_call`、`agent:tool_result` 事件已定义

---

## 二、设计目标

1. **明确分层**：Tools 是轻量级基础操作，Skills 是重量级专业能力
2. **内置能力不可禁用**：Tools 和内置 Skills 始终可用，保证 Agent 的完整能力
3. **自定义 Skills 可控**：用户自定义的 Skills 可由用户启用/禁用
4. **LLM 友好**：Tools 和 Skills 的定义统一发送给 LLM，LLM 无需区分二者
5. **前端清晰**：设置页面分开展示 Tools、内置 Skills 和自定义 Skills
6. **向后兼容**：现有功能不中断，渐进式迁移

---

## 三、架构设计

### 3.1 Tools 与 Skills 的定义对比

| 维度 | Tool（工具） | Skill（技能） |
|------|-------------|--------------|
| **定位** | 轻量级基础操作，Agent 的"手和眼" | 重量级专业能力，Agent 的"专业知识" |
| **复杂度** | 简单、原子化 | 复杂、可能多步骤 |
| **实现** | Rust 原生，无外部依赖 | 可能依赖 Sidecar (Python) |
| **频率** | 极高（每次对话几乎都会用到） | 中低（按需使用） |
| **用户控制** | 始终启用，不可禁用 | 内置 Skill 始终启用；自定义 Skill 可启用/禁用 |
| **风险等级** | 低（主要是读取/查询操作） | 可能高（修改/删除/生成） |
| **前端展示** | 设置页"工具"区域（只读展示） | 设置页"技能"区域（内置只读，自定义可切换） |
| **扩展方式** | 代码内置，随版本更新 | 内置 + 用户自定义 |

### 3.2 迁移方案：从现有 Skill 中拆分

#### 迁移为 Tool 的 Skill（3 个）

| 原 Skill | 新 Tool | 理由 |
|----------|---------|------|
| `list_workspace` | `list_directory` | 纯 Rust 原生，简单文件系统操作，调用频率极高 |
| `search_documents` | `search_files` | 纯 Rust 原生，简单文件搜索，调用频率极高 |
| `delete_document` | `delete_file` | 纯 Rust 原生，虽然高风险但逻辑简单 |

#### 保留为 Skill 的（4 个）

| Skill | 理由 |
|-------|------|
| `generate_document` | 依赖 Sidecar，复杂度高，涉及多格式文档生成 |
| `modify_document` | 依赖 Sidecar，复杂度高，涉及文档结构修改 |
| `convert_format` | 依赖 Sidecar，复杂度高，涉及跨格式转换 |
| `batch_process` | 依赖 Sidecar，编排多个操作，复杂度最高 |

#### 拆分后同时存在 Tool 和 Skill 的（2 个）

| 原 Skill | 拆分后的 Tool | 拆分后的 Skill | 说明 |
|----------|-------------|---------------|------|
| `read_document` | `read_file` | `read_document` | Tool 读取纯文本/轻量文件；Skill 通过 Sidecar 解析结构化文档 |
| `analyze_document` | `file_info` | `analyze_document` | Tool 获取文件元数据；Skill 通过 Sidecar 深度分析文档结构 |

#### 新增 Tool（3 个）

| 新 Tool | 说明 |
|---------|------|
| `file_exists` | 检查文件/目录是否存在，Agent 决策前常用 |
| `create_directory` | 创建目录，生成文档前常需要先确保目录存在 |
| `write_text_file` | 写入纯文本文件（.txt / .md / .csv 等），不依赖 Sidecar |

### 3.3 最终 Tools 与 Skills 清单

#### Tools（8 个，始终启用）

| Tool 名称 | 描述 | 实现 |
|-----------|------|------|
| `list_directory` | 列出指定目录中的文件和子目录结构 | Rust 原生 |
| `search_files` | 在指定目录中搜索文件，支持按文件名或内容搜索 | Rust 原生 |
| `read_file` | 读取纯文本文件内容（.txt/.md/.csv/.json 等） | Rust 原生 |
| `file_info` | 获取文件元数据（大小、修改时间、类型等） | Rust 原生 |
| `file_exists` | 检查文件或目录是否存在 | Rust 原生 |
| `delete_file` | 删除指定文件（含安全校验和可选备份） | Rust 原生 |
| `create_directory` | 创建目录（支持递归创建） | Rust 原生 |
| `write_text_file` | 写入纯文本文件内容 | Rust 原生 |

#### Skills（6 个内置 + 用户自定义）

**内置 Skills（始终启用，不可禁用）**

| Skill 名称 | 描述 | 实现 |
|------------|------|------|
| `read_document` | 读取结构化文档内容（Word/Excel/PPT/PDF），提取文本、表格、属性 | Sidecar |
| `generate_document` | 生成新文档（Word/Excel/PPT/PDF/Markdown） | Sidecar |
| `modify_document` | 修改已有文档，支持文本替换、添加段落、添加表格等 | Sidecar |
| `convert_format` | 文档格式转换（Word 转 PDF 等） | Sidecar |
| `analyze_document` | 分析文档结构和统计信息（字数、段落数、标题层级等） | Sidecar |
| `batch_process` | 批量处理多个文档（批量转换/修改/分析） | Sidecar |

**自定义 Skills（可启用/禁用）**

用户可通过 JSON 配置文件创建自定义 Skills，这些 Skills 可以被启用或禁用。

### 3.4 Rust 后端架构变更

#### 新增 Tool trait 和 ToolRegistry

```
src-tauri/src/services/
├── tool/                    # 新增：Tool 系统
│   ├── mod.rs              # 模块导出
│   ├── trait.rs            # Tool trait 定义
│   ├── registry.rs         # ToolRegistry（工具注册表）
│   └── builtin.rs          # 内置 Tool 实现（8 个）
├── skill/                   # 现有：Skill 系统（保留）
│   ├── mod.rs
│   ├── registry.rs         # SkillRegistry（保留，移除已迁移的 Skill）
│   ├── builtin.rs          # 内置 Skill（保留 6 个）
│   └── custom.rs           # 自定义 Skill（保留）
└── agent/
    ├── executor.rs          # 修改：同时查找 Tool 和 Skill
    └── context.rs           # 修改：系统提示词更新
```

#### Tool trait 定义

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称（唯一标识）
    fn tool_name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 参数 JSON Schema
    fn parameters(&self) -> Value;

    /// 工具分类
    fn category(&self) -> &str {
        "filesystem"
    }

    /// 执行工具
    async fn execute(&self, params: Value) -> ToolResult;
}

/// 工具执行结果（与 SkillResult 格式一致，便于 AgentExecutor 统一处理）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub success: bool,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// 工具信息（用于前端展示）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    /// 工具始终为内置
    pub is_builtin: bool,  // 始终为 true
    /// 工具始终启用
    pub enabled: bool,     // 始终为 true
    pub version: String,
    pub params_schema: Option<Value>,
}
```

#### ToolRegistry 设计

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// 生成工具定义（与 SkillRegistry.tool_definitions() 格式一致）
    pub fn tool_definitions(&self) -> Vec<Value> { ... }

    /// 获取工具的 Arc 引用
    pub fn get_arc(&self, name: &str) -> Option<Arc<dyn Tool>> { ... }

    /// 列出所有工具信息
    pub fn list_tools(&self) -> Vec<ToolInfo> { ... }
}
```

#### SkillRegistry 简化（移除禁用逻辑）

由于内置 Skills 不再允许被禁用，SkillRegistry 的禁用逻辑仅用于自定义 Skills：

```rust
pub struct SkillRegistry {
    /// 内置 Skills（始终启用）
    builtin_skills: HashMap<String, Arc<dyn Skill>>,
    /// 自定义 Skills（可启用/禁用）
    custom_skills: HashMap<String, Arc<dyn Skill>>,
    /// 已禁用的自定义 Skill ID 集合
    disabled_custom_skills: HashSet<String>,
}

impl SkillRegistry {
    /// 注册内置 Skill（从 builtin.rs 调用）
    pub fn register_builtin(&mut self, skill: Box<dyn Skill>) {
        let name = skill.skill_name().to_string();
        self.builtin_skills.insert(name.clone(), Arc::from(skill));
    }

    /// 注册自定义 Skill（从 custom.rs 调用）
    pub fn register_custom(&mut self, skill: Box<dyn Skill>) {
        let name = skill.skill_name().to_string();
        self.custom_skills.insert(name.clone(), Arc::from(skill));
    }

    /// 生成工具定义（仅包含已启用的 Skills）
    pub fn tool_definitions(&self) -> Vec<Value> {
        // 内置 Skills 全部包含
        // 自定义 Skills 仅包含未禁用的
    }

    /// 切换自定义 Skill 启用/禁用状态
    /// 对内置 Skill 调用时返回错误
    pub fn toggle_custom_skill(&mut self, skill_id: &str, enabled: bool) -> Result<Vec<String>, Error> {
        if self.builtin_skills.contains_key(skill_id) {
            return Err(Error::new("内置 Skill 不可禁用"));
        }
        // 仅允许切换自定义 Skill
        if enabled {
            self.disabled_custom_skills.remove(skill_id);
        } else {
            self.disabled_custom_skills.insert(skill_id.to_string());
        }
        Ok(self.disabled_custom_skills.iter().cloned().collect())
    }

    /// 获取 Skill 的 Arc 引用（先查内置，再查自定义）
    pub fn get_arc(&self, name: &str) -> Option<Arc<dyn Skill>> {
        self.builtin_skills.get(name).cloned()
            .or_else(|| self.custom_skills.get(name).cloned())
    }

    /// 列出所有技能信息
    pub fn list_skills(&self) -> Vec<SkillInfo> {
        // 内置 Skills 的 enabled 字段始终为 true
        // 自定义 Skills 根据 disabled_custom_skills 判断
    }

    /// 注销自定义 Skill（删除时调用）
    pub fn unregister_custom(&mut self, skill_id: &str) -> bool {
        self.disabled_custom_skills.remove(skill_id);
        self.custom_skills.remove(skill_id).is_some()
    }
}
```

**迁移注意**：
- 现有 `register()` 方法需拆分为 `register_builtin()` 和 `register_custom()`
- 现有 `with_disabled_skills()` 初始化时需过滤掉内置 Skill 的 ID（已有用户可能禁用了内置 Skill，需在迁移时清除）
- `toggle_skill()` 命令需改为调用 `toggle_custom_skill()`，对内置 Skill 返回错误

#### AgentExecutor 变更

```rust
pub struct AgentExecutor<R: Runtime> {
    router: Arc<LlmRouter>,
    tool_registry: Arc<ToolRegistry>,      // 新增
    skill_registry: Arc<Mutex<SkillRegistry>>,
    emitter: AgentEmitter<R>,
    confirm_channels: Arc<Mutex<HashMap<String, ...>>>,
    // ...
}
```

**执行流程变更**：

```
1. 合并 Tool + Skill 定义 → 统一发送给 LLM
2. LLM 返回 tool_calls
3. 按 tool_call.name 查找：
   a. 先查 ToolRegistry（基础操作优先）
   b. 再查 SkillRegistry（高级技能）
4. 执行找到的 Tool 或 Skill
5. 返回结果给 LLM
```

#### AppState 变更

```rust
pub struct AppState {
    db: Arc<Database>,
    config: Arc<Mutex<ConfigManager>>,
    active_agents: Arc<Mutex<HashMap<String, bool>>>,
    confirm_channels: Arc<Mutex<HashMap<String, ...>>>,
    doc_service: Arc<DocumentService>,
    llm_router: Arc<RwLock<Arc<LlmRouter>>>,
    tool_registry: Arc<ToolRegistry>,       // 新增
    skill_registry: Arc<Mutex<SkillRegistry>>,
    custom_skill_loader: Arc<CustomSkillLoader>,
    fs_watcher: Arc<FsWatcherService>,
}
```

### 3.5 前端架构变更

#### 类型定义变更

```typescript
// types/settings.ts 新增

export interface ToolInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  isBuiltin: true;       // 工具始终为内置
  enabled: true;          // 工具始终启用
  version: string;
  paramsSchema?: unknown;
}

// SkillInfo 类型更新
export interface SkillInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  isBuiltin: boolean;     // true 表示内置，false 表示自定义
  enabled: boolean;       // 内置 Skill 此字段始终为 true；自定义 Skill 可为 true/false
  version: string;
  paramsSchema?: unknown;
  supportedTypes: string[];
}
```

#### SkillsTab 拆分

将现有的 `SkillsTab` 拆分为三个区域：

1. **工具区域**（只读展示）：列出所有内置 Tool，不可禁用
2. **内置技能区域**（只读展示）：列出内置 Skill，始终启用，不可禁用
3. **自定义技能区域**（可增删改查 + 可切换）：保留现有自定义 Skill 功能，可启用/禁用

#### 工作流节点

`ToolNode` 组件无需变更，因为 Tool 和 Skill 在 LLM 侧都是 `tool_call`，前端展示逻辑一致。

### 3.6 Tauri 命令变更

| 命令 | 变更 |
|------|------|
| `list_skills` | 保留，返回所有 Skill 列表（内置 + 自定义），内置 Skill 的 enabled 字段始终为 true |
| `list_tools` | **新增**，返回 Tool 列表 |
| `toggle_skill` | 保留，仅控制自定义 Skill（内置 Skill 调用时返回错误或忽略） |
| `add_custom_skill` | 保留 |
| `update_custom_skill` | 保留 |
| `delete_custom_skill` | 保留 |

### 3.7 系统提示词变更

当前系统提示词（[context.rs](../src-tauri/src/services/agent/context.rs)）需要更新，明确告知 LLM Tools 和 Skills 的区别：

```
你是 DocAgent，一个专业的 AI 文档处理助手。

你可以使用以下两类能力：

工具（Tools）- 基础文件操作，始终可用：
- list_directory: 列出目录内容
- search_files: 搜索文件
- read_file: 读取文本文件
- file_info: 获取文件信息
- file_exists: 检查文件是否存在
- delete_file: 删除文件
- create_directory: 创建目录
- write_text_file: 写入文本文件

技能（Skills）- 专业文档处理，按需调用：
- read_document: 读取结构化文档
- generate_document: 生成新文档
- modify_document: 修改已有文档
- convert_format: 转换文档格式
- analyze_document: 分析文档结构
- batch_process: 批量处理文档

使用原则：
1. 简单的文件操作优先使用工具
2. 需要解析/生成结构化文档时使用技能
3. 读取 .txt/.md/.csv 等纯文本文件用 read_file
4. 读取 .docx/.xlsx/.pptx/.pdf 等结构化文档用 read_document
...
```

---

## 四、实施计划

### 阶段一：后端 Tool 系统基础（预计 2-3 天）

#### 任务 1.1：创建 Tool trait 和 ToolRegistry

**文件**：
- 创建：`src-tauri/src/services/tool/mod.rs`
- 创建：`src-tauri/src/services/tool/trait.rs`
- 创建：`src-tauri/src/services/tool/registry.rs`

**内容**：
- 定义 `Tool` trait（`tool_name`, `description`, `parameters`, `category`, `execute`）
- 定义 `ToolResult` 结构体（与 `SkillResult` 格式一致）
- 定义 `ToolInfo` 结构体
- 实现 `ToolRegistry`（注册、查找、生成工具定义、列出工具信息）

#### 任务 1.2：实现 8 个内置 Tool

**文件**：
- 创建：`src-tauri/src/services/tool/builtin.rs`

**内容**：
- `ListDirectoryTool` - 从 `ListWorkspaceSkill` 迁移，逻辑基本不变
- `SearchFilesTool` - 从 `SearchDocumentsSkill` 迁移，逻辑基本不变
- `ReadFileTool` - 新增，读取纯文本文件内容
- `FileInfoTool` - 新增，获取文件元数据
- `FileExistsTool` - 新增，检查文件/目录是否存在
- `DeleteFileTool` - 从 `DeleteDocumentSkill` 迁移，逻辑基本不变
- `CreateDirectoryTool` - 新增，创建目录
- `WriteTextFileTool` - 新增，写入纯文本文件

**各 Tool 参数 Schema**：

```json
// list_directory
{ "type": "object", "properties": { "path": { "type": "string", "description": "目录路径" }, "depth": { "type": "integer", "description": "遍历深度，默认1", "default": 1 }, "extensions": { "type": "array", "items": { "type": "string" }, "description": "筛选文件扩展名" } } }

// search_files
{ "type": "object", "properties": { "query": { "type": "string", "description": "搜索关键词" }, "directory": { "type": "string", "description": "搜索目录路径" }, "extensions": { "type": "array", "items": { "type": "string" }, "description": "限定文件扩展名" }, "include_content": { "type": "boolean", "description": "是否搜索文件内容", "default": false }, "max_results": { "type": "integer", "description": "最大结果数", "default": 50 } } }

// read_file
{ "type": "object", "properties": { "path": { "type": "string", "description": "文件路径" }, "encoding": { "type": "string", "description": "文件编码，默认utf-8", "default": "utf-8" }, "max_size": { "type": "integer", "description": "最大读取字节数，默认1MB", "default": 1048576 } }, "required": ["path"] }

// file_info
{ "type": "object", "properties": { "path": { "type": "string", "description": "文件路径" } }, "required": ["path"] }

// file_exists
{ "type": "object", "properties": { "path": { "type": "string", "description": "文件或目录路径" } }, "required": ["path"] }

// delete_file
{ "type": "object", "properties": { "path": { "type": "string", "description": "要删除的文件路径" }, "create_backup": { "type": "boolean", "description": "删除前是否创建备份", "default": true } }, "required": ["path"] }

// create_directory
{ "type": "object", "properties": { "path": { "type": "string", "description": "目录路径" }, "recursive": { "type": "boolean", "description": "是否递归创建父目录", "default": true } }, "required": ["path"] }

// write_text_file
{ "type": "object", "properties": { "path": { "type": "string", "description": "文件路径" }, "content": { "type": "string", "description": "文件内容" }, "encoding": { "type": "string", "description": "文件编码，默认utf-8", "default": "utf-8" }, "append": { "type": "boolean", "description": "是否追加写入", "default": false } }, "required": ["path", "content"] }
```

**注意**：`write_text_file` 仅处理纯文本文件（.txt/.md/.csv/.json 等），不替代 `generate_document` Skill 的 Markdown 生成功能。`generate_document` 通过 Sidecar 生成 Markdown 时可能包含额外的格式处理（如 front matter、目录结构等），两者功能互补。

#### 任务 1.3：修改 AgentExecutor 支持 Tool + Skill 双注册表

**文件**：
- 修改：`src-tauri/src/services/agent/executor.rs`

**内容**：
- 添加 `tool_registry: Arc<ToolRegistry>` 字段（ToolRegistry 不需要 Mutex，因为工具在运行时不会增删）
- 修改 `new()` 构造函数，接收 `tool_registry` 参数
- 修改 `execute()` 方法：合并 Tool 和 Skill 的工具定义
- 修改 tool_call 执行逻辑：先查 ToolRegistry，再查 SkillRegistry
- 修改高风险操作判断逻辑（`delete_file` 也需要确认，原 `delete_document` 已迁移）
- 修改 `workspace_root` 注入逻辑（Tool 也需要，如 `list_directory`、`search_files` 等）
- 添加旧 Skill 名称映射兼容层：
  ```rust
  /// 旧 Skill 名称到新 Tool 名称的映射（向后兼容历史会话）
  const SKILL_NAME_MIGRATION_MAP: &[(&str, &str)] = &[
      ("list_workspace", "list_directory"),
      ("search_documents", "search_files"),
      ("delete_document", "delete_file"),
  ];
  
  fn resolve_tool_name(name: &str) -> &str {
      SKILL_NAME_MIGRATION_MAP.iter()
          .find(|(old, _)| *old == name)
          .map(|(_, new)| *new)
          .unwrap_or(name)
  }
  ```

#### 任务 1.4：修改 AppState 和初始化流程

**文件**：
- 修改：`src-tauri/src/lib.rs`

**内容**：
- AppState 添加 `tool_registry: Arc<ToolRegistry>` 字段
- 初始化时创建 ToolRegistry 并注册内置 Tool
- 修改 `start_agent` 命令，将 ToolRegistry 传入 AgentExecutor
- 修改 `with_disabled_skills()` 初始化逻辑：过滤掉内置 Skill 的 ID，仅保留自定义 Skill 的禁用状态

#### 任务 1.5：精简 SkillRegistry

**文件**：
- 修改：`src-tauri/src/services/skill/builtin.rs`

**内容**：
- 移除已迁移为 Tool 的 Skill：`list_workspace`、`search_documents`、`delete_document`
- 保留 6 个 Skill：`read_document`、`generate_document`、`modify_document`、`convert_format`、`analyze_document`、`batch_process`

#### 任务 1.6：添加 Tauri 命令

**文件**：
- 修改：`src-tauri/src/commands/skill.rs`（或新建 `src-tauri/src/commands/tool.rs`）
- 修改：`src-tauri/src/lib.rs`（注册新命令）

**内容**：
- 新增 `list_tools` 命令：
  ```rust
  #[tauri::command]
  pub async fn list_tools(state: State<'_, AppState>) -> Result<Vec<ToolInfo>, CommandError> {
      Ok(state.tool_registry.list_tools())
  }
  ```
- 修改 `list_skills` 命令（仅返回 Skill 列表，内置 Skill 的 enabled 字段始终为 true）
- 修改 `toggle_skill` 命令：对内置 Skill 调用时返回错误而非静默忽略

### 阶段二：前端适配（预计 1-2 天）

#### 任务 2.1：更新前端类型定义

**文件**：
- 修改：`src/types/settings.ts`
- 修改：`src/types/index.ts`（如有需要）

**内容**：
- 新增 `ToolInfo` 类型
- 确保 `SkillInfo` 类型不变

#### 任务 2.2：更新 Tauri 服务层

**文件**：
- 修改：`src/services/tauri.ts`

**内容**：
- 新增 `listTools()` 函数

#### 任务 2.3：重构 SkillsTab 组件

**文件**：
- 修改：`src/components/settings/SkillsTab.tsx`

**内容**：
- 拆分为三个区域：工具（只读）、内置技能（可切换）、自定义技能（可增删改查）
- 工具区域调用 `listTools()` 获取数据
- 技能区域继续调用 `listSkills()` 获取数据

#### 任务 2.4：更新 Settings Store

**文件**：
- 修改：`src/stores/useSettingsStore.ts`

**内容**：
- 新增 `tools` 状态
- 新增 `refreshTools()` 方法

### 阶段三：系统提示词与文档更新（预计 1 天）

#### 任务 3.1：更新系统提示词

**文件**：
- 修改：`src-tauri/src/services/agent/context.rs`

**内容**：
- 更新 `build_system_prompt()` 函数，明确区分 Tools 和 Skills
- 添加工具使用优先级指导

#### 任务 3.2：更新错误码

**文件**：
- 修改：`src-tauri/src/errors.rs`

**内容**：
- 新增 Tool 相关错误码段（9000-9999，因 8000-8999 已被更新相关错误码占用）
- 示例错误码：
  - 9001: TOOL_NOT_FOUND（工具不存在）
  - 9002: TOOL_EXECUTION_ERROR（工具执行错误）
  - 9003: TOOL_INVALID_PARAMS（工具参数无效）

#### 任务 3.3：更新开发文档

**文件**：
- 修改：`docs/tech_architecture.md`
- 修改：`docs/tauri_commands.md`
- 修改：`docs/skill_development.md`
- 修改：`CLAUDE.md`

---

## 五、关键设计决策

### 5.1 为什么 Tool 和 Skill 使用不同的 trait？

虽然二者接口相似（都有 name/description/parameters/execute），但使用不同 trait 的原因：

1. **语义清晰**：代码中能明确区分哪些是基础工具，哪些是高级技能
2. **独立演进**：Tool 和 Skill 可能会有不同的扩展方向（如 Tool 可能增加缓存机制，Skill 可能增加流式输出）
3. **注册表隔离**：ToolRegistry 不需要禁用逻辑；SkillRegistry 的禁用逻辑仅用于自定义 Skills
4. **类型安全**：避免在同一个注册表中混淆 Tool 和 Skill

### 5.2 为什么内置 Skills 不可禁用？

内置 Skills（如 generate_document、modify_document）是 Agent 的核心能力，禁用它们会导致 Agent 无法完成基本的文档处理任务。用户如果不需要某些功能，可以通过自定义 Skills 来扩展而非限制内置能力。

只有用户自定义的 Skills 可以被启用/禁用，因为这些是用户主动添加的能力，用户有权决定是否使用。

### 5.3 为什么 delete_file 是 Tool 而不是 Skill？

虽然删除文件是高风险操作，但它的逻辑非常简单（路径校验 + 文件删除 + 可选备份），不需要 Sidecar，属于基础文件系统操作。高风险通过 executor 的确认机制处理，不应该是区分 Tool/Skill 的标准。

### 5.4 为什么 read_file 和 read_document 同时存在？

- `read_file`（Tool）：读取纯文本文件（.txt/.md/.csv/.json/.xml 等），Rust 原生实现，速度快，无需 Sidecar
- `read_document`（Skill）：读取结构化文档（.docx/.xlsx/.pptx/.pdf），需要 Sidecar 解析文档结构

LLM 的系统提示词会明确指导：纯文本文件用 `read_file`，结构化文档用 `read_document`。

### 5.5 为什么新增 write_text_file Tool？

当前生成 Markdown 文件也需要走 Sidecar 的 `generate_document`，但实际上 Markdown 是纯文本格式，直接用 Rust 写入即可，无需启动 Python 进程。这能显著提升 Markdown 文件的生成速度。

### 5.6 Tool 定义和 Skill 定义如何合并发送给 LLM？

```rust
// 在 AgentExecutor.execute() 中
let tool_defs: Vec<Value> = self.tool_registry.tool_definitions();
let skill_defs: Vec<Value> = {
    let reg = self.skill_registry.lock().await;
    reg.tool_definitions()
};
let all_defs: Vec<ToolDefinition> = [tool_defs, skill_defs].concat()
    .iter()
    .filter_map(|v| {
        // 转换为 ToolDefinition...
    })
    .collect();

// 传给 LLM
let mut stream_rx = self.router.chat_stream(&messages, &all_defs).await?;
```

LLM 看到的是统一的 tools 列表，无需区分 Tool 和 Skill。

---

## 六、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| LLM 混淆 read_file 和 read_document | 读取文档时用了错误的工具 | 系统提示词明确指导 + 工具描述中注明适用文件类型 |
| 迁移后 Skill 名称变化导致历史会话异常 | 旧会话中 LLM 调用已不存在的 Skill | executor 中添加名称映射兼容层 |
| Tool 数量增多导致 LLM 上下文变长 | Token 消耗增加 | 8 个 Tool 的定义总共约 800 token，影响可控 |
| 用户无法禁用内置能力（Tool 和内置 Skill） | 用户可能希望限制某些高风险操作 | 高风险操作仍需用户确认，确认机制不受影响；用户可通过自定义 Skill 扩展而非限制 |

---

## 七、验收标准

1. 8 个内置 Tool 正常注册和执行
2. 6 个内置 Skill 正常注册和执行，始终启用，不可禁用
3. 自定义 Skill 可正常创建、编辑、删除、启用/禁用
4. Agent 执行器正确合并 Tool 和 Skill 定义发送给 LLM
5. LLM 返回 tool_call 时，executor 正确路由到 Tool 或 Skill
6. 前端设置页面正确展示工具、内置技能、自定义技能三个区域
7. 工具区域和内置技能区域为只读展示，自定义技能区域可切换启用/禁用
8. toggle_skill 命令仅对自定义 Skill 有效，对内置 Skill 调用时返回错误
9. 高风险操作（delete_file、modify_document 等）仍需用户确认
10. 历史会话中的旧 Skill 名称（list_workspace、search_documents、delete_document）能正确映射到新 Tool
11. 已有用户禁用内置 Skill 的配置在迁移后被自动清除

---

## 八、与其他计划的依赖关系

本计划应先于《Skills 整合开发计划》执行，原因如下：

1. **架构先行**：本计划重构 Skill/Tool 分层架构，是后续功能扩展的基础
2. **避免重复工作**：如果先执行 Skills 整合（扩展参数、增强 Handler），再执行分离重构，已扩展的参数 Schema 需要重新调整
3. **系统提示词统一**：两份计划都修改 `context.rs` 的系统提示词，应在本计划中先建立 Tools/Skills 分层提示词框架，再在整合计划中添加文档设计指导
4. **SkillRegistry 简化**：本计划将 SkillRegistry 拆分为 builtin/custom 两部分，整合计划在此基础上扩展参数更清晰
