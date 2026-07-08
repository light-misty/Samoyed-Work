# DocAgent 写作型 Agent 改造计划

> **改造目标**：将当前以"文档处理"为核心的 DocAgent 改造为以"文本创作"为核心的写作型 Agent 应用，支持一句话生成长篇小说、科研论文、记叙文、议论文等。
>
> **本文档定位**：基于对项目源码的全面深度分析 + 对前沿写作 Agent 工具（Sudowrite/NovelAI/NovelCrafter/PaperOrchestra/Agents' Room）和知名通用 Agent（Claude Code/Aider/OpenHands/OpenCode/Codex）的联网调研，制定的总改造设计文档。
>
> **执行原则**：遵循 superpowers 框架的 brainstorming → writing-plans → executing-plans 流程，本文档处于 brainstorming 阶段产出，待用户确认关键决策点后，进入 writing-plans 阶段产出 TDD 化的任务清单。

---

**文档版本**: v1.0
**创建日期**: 2026-07-08
**项目仓库**: d:\DeskTop\DocAgent
**理论依据**:
- Anthropic《Effective Context Engineering for AI Agents》（2025-09-29）
- Anthropic《Building Effective Agents》（2024-12-19）
- 清华 LongWriter（AgentWrite 管道）
- Google DeepMind Agents' Room（ICLR 2025）
- Google PaperOrchestra（多智能体论文写作）

---

## 第一部分：现状评估

### 1.1 已具备的优势（基础设施层已达到生产级水准）

经对项目源码的全面深度分析（7 个核心文件、约 4800 行 Rust 代码），DocAgent 在工程基础设施层已具备以下成熟能力：

| 能力 | 代码位置 | 评价 |
|------|---------|------|
| 7 层分层系统提示词架构 | [context.rs:791-831](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) | Layer 0-7 清晰分离，Layer 6/7 按 Token 预算动态注入 |
| Token 预算动态分配 | [token_budget.rs:46-57](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/token_budget.rs) | system 15% / tools 10% / conversation 50% / response 25% |
| Scratchpad 结构化笔记 | [context.rs:100-110](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) | 严格遵循 Anthropic Structured Note-taking 模式 |
| 增量持久化 | [executor.rs:291-300](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/executor.rs) | 每轮迭代后立即写库，崩溃时最多丢失一轮 |
| 截断重试机制 | [executor.rs:822-1029](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/executor.rs) | max_tokens 翻倍重试，最多 2 次 |
| 缓存友好设计 | [context.rs:498-510](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) | 稳定前缀 + Scratchpad 末尾注入，最大化 LLM 缓存命中 |
| reasoning_content 压缩 | [context.rs:533-554](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) | 早期推理内容截断（1200→500 字符） |
| 任务类型识别 | [task_type.rs:31-99](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/task_type.rs) | 关键词匹配基础设施可复用 |
| 多 Provider 适配 | [src-tauri/src/services/llm/](file:///d:/DeskTop/DocAgent/src-tauri/src/services/llm/) | OpenAI/Anthropic/Gemini/Ollama + 健康检查 + Fallback |
| 长上下文模型预设 | [context_presets.rs:24-158](file:///d:/DeskTop/DocAgent/src-tauri/src/services/llm/context_presets.rs) | 70+ 模型预设，含 1M-10M 长上下文 |
| 流式输出 + 工具调用 | [executor.rs:600-740](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/executor.rs) | 流式增量合并 + 提前发射 tool_call 事件 |
| 版本快照机制 | [agent.rs:1085-1196](file:///d:/DeskTop/DocAgent/src-tauri/src/commands/agent.rs) | write_text_file 覆盖前自动备份 |
| Python Sidecar 文档处理 | [sidecar/handlers/](file:///d:/DeskTop/DocAgent/sidecar/handlers/) | Word/Excel/PPT/PDF/Markdown 全格式 |
| 用户确认机制 | [executor.rs:302-396](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/executor.rs) | oneshot channel + 5 分钟超时 + 三级确认 |

### 1.2 核心差距（写作领域知识层完全空白）

#### 差距 1：身份定位完全错位

[context.rs:834-855](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) 的 `layer_identity()` 写死"DocAgent，AI 文档处理专家"，明确宣称"精通五大文档格式的生成、读取、修改、格式转换与结构分析"。这与写作型 Agent 完全不兼容——LLM 会把"写一篇科幻小说"理解为"生成一个 .docx 文件"而非"创作叙事内容"。

#### 差距 2：任务类型枚举缺失写作类型

[task_type.rs:8-26](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/task_type.rs) 的 `TaskType` 枚举仅有 `Docx/Xlsx/Pptx/Pdf/Markdown/FileSystem/General/Unknown`，**完全没有 Novel/Essay/Paper/Argumentative 等写作类型**。用户说"写一篇科幻小说"会被识别为 `Unknown`，不注入任何设计规范。

#### 差距 3：滑动窗口上限 10 轮

[token_budget.rs:139](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/token_budget.rs) 的 `window.clamp(2, 10)` 限制最多保留 10 轮完整对话（约 40 条消息）。生成 50 章长篇小说时，第 1 章的人物设定在第 11 章时已被压缩为摘要占位符。

#### 差距 4：摘要占位过于简陋

[context.rs:628-636](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs) 的摘要占位仅是一句话 `"[系统摘要: 已省略 N 条早期对话消息]"`，**没有任何内容摘要**。LLM 完全不知道被省略了什么人物细节、伏笔、对话风格。

#### 差距 5：Scratchpad 容量与结构不足

- [builtin.rs:3264](file:///d:/DeskTop/DocAgent/src-tauri/src/services/tool/builtin.rs)：单条笔记上限 **500 字符**，无法存储完整角色档案
- 仅支持 add/read/clear 三种操作，**无法修改/删除单条笔记**
- 仅支持纯文本 content 字段，**无结构化分类**（如角色/世界观/伏笔/章节）

#### 差距 6：缺少写作专用 Handler/Tool

[executor.rs:1129-1148](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/executor.rs) 的 16 个 Tool + 4 个 Handler 全是文件系统/文档操作，**完全没有**：
- `outline_manager`（大纲管理）
- `character_profile`（角色档案）
- `worldbuilding_manager`（世界观管理）
- `plot_graph`（剧情图谱）
- `foreshadow_tracker`（伏笔追踪）
- `consistency_checker`（一致性检查）
- `style_analyzer`（风格分析）

#### 差距 7：会话级状态不持久化

[builtin.rs:18-21](file:///d:/DeskTop/DocAgent/src-tauri/src/services/tool/builtin.rs) 的 `SharedScratchpadStates` 是 `Arc<RwLock<HashMap>>` 内存态，**会话结束/应用重启即丢失**。长篇小说通常跨多日、多会话创作，每次重启需用户复述或 Agent 重新读取工作区文件。

#### 差距 8：跨会话摘要注入已被禁用

[agent.rs:862-864](file:///d:/DeskTop/DocAgent/src-tauri/src/commands/agent.rs) 明确注释：
```rust
// 历史会话摘要注入已禁用
// 用户明确要求：新对话中不应该存在上文，每个会话应该是完全独立的
```

虽然数据库 `session_summaries` 表仍在写入，但新会话启动时不读取注入。**对长篇小说创作的致命影响**：每开一个新会话（如"续写第 51 章"），Agent 对前 50 章的人物、剧情、设定一无所知。

#### 差距 9：max_tokens 上限不足

[executor.rs:48](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/executor.rs) 的 `MAX_TOKENS_CEILING = 131072`（约 6-10 万中文汉字），不足以单次生成 10 万字小说，必须分章节生成。

#### 差距 10：工具结果 6000 字符截断

[executor.rs:52](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/executor.rs) 的 `MAX_TOOL_RESULT_CHARS = 6000` 会截断长文回读，章节正文（通常 3000-10000 字）会被截断为头 4200 + 尾 1800 字符，**无法检查伏笔连贯性**。

#### 差距 11：文档设计指南全是工程实现细节

[document_design.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/document_design.rs) 的 4 类指南全是配色方案、字体规范、API 调用陷阱，**完全没有叙事结构、文风控制、章节连贯性指导**。

#### 差距 12：缺少 Subagent 机制

Claude Code 的 Task 工具是其长任务能力的核心——子 Agent 在独立上下文中运行，主 Agent 只接收摘要结果。**DocAgent 完全没有 Subagent 机制**，所有迭代共用一个上下文窗口，长篇创作必然导致上下文爆炸。

#### 差距 13：缺少 TodoWrite 任务规划

Claude Code 的 TodoWrite 工具用于"防止 Agent 跑偏"，显式展示规划步骤。**DocAgent 没有任务规划工具**，长篇创作的多阶段流程（大纲→角色→章节→润色）无法显式管理。

---

## 第二部分：前沿工具调研结论

### 2.1 专业写作 Agent 工具核心架构对比

| 维度 | Sudowrite | NovelAI | NovelCrafter | **DocAgent 现状** |
|------|-----------|---------|--------------|------------------|
| **核心架构** | Story Bible（模板化设定库） | Lorebook（关键词触发注入） | Codex（结构化知识库+混合检索） | **无任何设定管理** |
| **记忆持久性** | 会话级 | 半自动触发 | 手动 Codex | **会话级（内存态）** |
| **角色管理** | 完整 | 完整 | 完整（双层：稳定+变化） | **无** |
| **世界观** | 完整 | 完整（Memory Book） | 完整 | **无** |
| **章节管理** | 完整 | 部分 | 完整 | **无** |
| **大纲规划** | 完整 | 部分 | 完整（网格/矩阵/大纲三视图） | **无** |
| **伏笔追踪** | 有 | 无 | 无 | **无** |
| **混合检索** | 无 | 关键词匹配 | BM25 + 向量 | **无** |
| **续写/改写/描写** | 专用模式 | 续写模式 | 完整 | **无** |

### 2.2 知名通用 Agent 架构核心模式

| 工具/模式 | 核心机制 | 对 DocAgent 改造的启示 |
|-----------|---------|---------------------|
| **Claude Code Task 工具** | Subagent 在独立上下文中运行，主 Agent 只接收摘要 | **最关键缺口**——长篇写作"上下文不爆炸"的根本保障 |
| **Claude Code TodoWrite** | 任务规划与防跑偏，显式展示规划步骤 | 写作流程多阶段（大纲→章节→润色）需要显式管理 |
| **Claude Code Skill 系统** | 自动触发可复用流程（brainstorming/TDD/debugging） | 写作场景可设计"大纲生成"/"角色创建"/"章节评审"等专用 Skill |
| **Anthropic Compaction** | 接近窗口上限时高保真摘要，重启新上下文 | 解决 50+ 章长篇创作的上下文爆炸 |
| **Anthropic Structured Note-taking** | Agent 自主维护结构化笔记 | DocAgent 已实现（Scratchpad），需扩展为 Story Map |
| **Anthropic JIT 检索** | 维护轻量标识符，运行时用工具动态加载 | 替代"全量塞入上下文"，用 search_memory 工具按需检索 |
| **Aider Repo Map** | tree-sitter 解析符号，按相关性排序注入 | 构建"Story Map"——解析角色/地点/时间线/伏笔，按相关性注入 |
| **Aider Architect Mode** | 架构师模型（reasoning）+ 编辑器模型分离 | 长篇写作分离"剧情架构师"+"章节写手" |
| **OpenHands CodeAct** | 用可执行代码做一切 Action | 写作场景可用 Python 脚本做查重、统计、风格分析 |
| **Anthropic Prompt Chaining** | 任务分解为顺序步骤，每步处理上一步输出 | 大纲→评审 gate→正文生成→润色（写作标准 workflow） |
| **Anthropic Evaluator-Optimizer** | 一个 LLM 生成、另一个评估反馈，循环迭代 | 文学润色核心模式——生成→批评→修改循环 |
| **LongWriter AgentWrite** | 计划阶段生成段落大纲（200-1000 字/段）+ 串行书写 | 突破单次 2000 字限制，扩展到 20000+ 字 |
| **PaperOrchestra 5 智能体** | 大纲→绘图→文献→章节→优化接力 | 科研论文生成的标准流水线 |
| **Agents' Room 编剧室** | 规划 Agent（冲突/角色/设定/情节）+ 写作 Agent（5 幕结构）+ Scratchpad 共享 | 长篇小说创作的多 Agent 协作模式 |

### 2.3 长上下文与记忆管理技术对比

| 技术 | 原理 | DocAgent 适用性 |
|------|------|----------------|
| **MemGPT 虚拟内存分页** | OS 启发，主上下文（RAM）+ 外存（Disk）+ 召回记忆 | 可参考，但实现复杂 |
| **Compaction 高保真摘要** | 接近窗口上限时压缩 | **推荐**——Rust 端可实现，不依赖外部框架 |
| **向量检索（Chroma）** | 嵌入式、极简安装、Python API | **推荐**——Python Sidecar 集成，零额外服务 |
| **混合检索（BM25+向量+KG）** | 三路召回 + 融合排序 | 长篇（>20 万字）时使用，KG 仅动感叙事有效 |
| **时序衰减+重要性加权** | Recency × Importance × Relevance | 检索评分公式，可参考 Generative Agents |
| **上下文工程 7 类输入** | 大纲/衔接/人物/世界观/伏笔/摘要/检索按优先级 | **强烈推荐**——按优先级注入结构化输入 |

### 2.4 写作专用提示词技术

| 技术 | 原理 | 写作应用 |
|------|------|---------|
| **Self-Refine** | LLM 生成 → 自我反馈 → 改进，迭代循环 | 初稿→自评→修改，平均提升 20% |
| **Chain of Density** | 从稀疏到密集，5 次迭代摘要 | 大纲密度链——粗略大纲逐轮补充细节 |
| **Outline-Driven Generation** | 先大纲→检查→基于大纲写正文 | 写作标准 workflow |
| **DOC 框架** | 故意大纲创建（含角色弧光、三幕结构） | 长篇结构化创作 |
| **Re3 框架** | Rephrase-Reflect-Revise 迭代 | 长篇故事生成 |
| **角色一致性（Lost in Stories）** | 实体状态追踪 + 结构化角色档案 + 检索注入 | 解决长篇角色崩坏 |

---

## 第三部分：差距分析矩阵

### 3.1 综合差距矩阵

| 能力域 | 当前状态 | 前沿工具基准 | 差距等级 | 改造优先级 |
|--------|---------|-------------|---------|-----------|
| **System Prompt 身份** | "AI 文档处理专家" | "小说家/学术作者" | 严重 | P0 |
| **TaskType 枚举** | 仅文档格式 | Novel/Essay/Paper/Argumentative | 严重 | P0 |
| **任务识别关键词** | 仅文档格式 | 写作类型关键词 | 严重 | P0 |
| **文档设计指南** | 配色/字体/API 陷阱 | 叙事结构/三幕剧/IMRAD | 严重 | P0 |
| **Subagent 机制** | 无 | Task 工具（核心缺口） | 严重 | P0 |
| **TodoWrite 任务规划** | 无 | 显式任务清单 | 严重 | P0 |
| **Scratchpad 持久化** | 内存态 | 数据库持久化 | 严重 | P1 |
| **Scratchpad 容量** | 500 字符 | 2000+ 字符 + 结构化字段 | 中等 | P1 |
| **跨会话摘要注入** | 已禁用 | 可配置启用 | 严重 | P1 |
| **角色档案系统** | 无 | 完整（双层：稳定+变化） | 严重 | P1 |
| **世界观知识库** | 无 | Lorebook/Codex | 严重 | P1 |
| **伏笔追踪** | 无 | 生命周期管理 | 中等 | P1 |
| **章节大纲持久化** | 无 | 多层大纲（卷/章/节） | 严重 | P1 |
| **滑动窗口上限** | 10 轮 | 50-100 轮（长上下文时） | 中等 | P1 |
| **摘要占位** | "已省略 N 条" | LLM 高保真摘要 | 严重 | P1 |
| **max_tokens 上限** | 131072 | 262144+（写作模式） | 中等 | P1 |
| **工具结果截断** | 6000 字符 | 12000+（写作场景） | 中等 | P1 |
| **max_iterations** | 100 | 500+（写作型 Agent） | 中等 | P1 |
| **一致性检查工具** | 无 | 角色/时间线/伏笔检查 | 中等 | P2 |
| **风格分析工具** | 无 | 句长/词汇/节奏分析 | 中等 | P2 |
| **向量检索** | 无 | Chroma 集成 | 低 | P2 |
| **Chain of Density 大纲** | 无 | 5 轮密度链精炼 | 低 | P2 |
| **Self-Refine 评审** | 无 | 生成→批评→修改循环 | 低 | P2 |
| **多 Agent 角色化** | 无 | 主编/写手/编辑协作 | 低 | P3 |
| **知识图谱检索** | 无 | 多跳关联查询 | 低 | P3（仅动感叙事） |

### 3.2 核心瓶颈识别

经分析，长篇小说生成的三大核心瓶颈是：

1. **上下文爆炸**：50+ 章长篇创作超出任何模型上下文窗口
   - **解决路径**：Subagent 隔离 + Compaction 压缩 + JIT 检索

2. **长程一致性**：人物/伏笔/世界观在多章节间保持一致
   - **解决路径**：Story Map 持久化 + 一致性检查工具 + Self-Refine 评审

3. **单次生成长度限制**：max_tokens 限制单次输出
   - **解决路径**：AgentWrite 分段串行生成 + 章节级持久化 + 断点续写

---

## 第四部分：改造目标与设计原则

### 4.1 改造目标

**总目标**：将 DocAgent 改造为支持"一句话生成长篇小说/科研论文/记叙文/议论文"的写作型 Agent 应用。

**具体目标**：

| 文体类型 | 目标长度 | 质量要求 |
|---------|---------|---------|
| 长篇小说 | 10-50 万字 | 人物一致、伏笔回收、文风统一、章节连贯 |
| 中短篇小说 | 1-10 万字 | 结构完整、情节合理、文风统一 |
| 科研论文 | 5000-20000 字 | IMRAD 结构、引用规范、逻辑严谨 |
| 记叙文 | 1000-5000 字 | 时间顺序、人物生动、情节清晰 |
| 议论文 | 1000-5000 字 | 论点明确、论据充分、逻辑严密 |

### 4.2 设计原则

基于 Anthropic 三大原则：

#### 原则一：Simplicity（简洁性）

- 优先在现有架构上叠加写作层，而非重写架构
- 不引入 Python 框架依赖（如 LangGraph/CrewAI），保持 Rust 架构纯净
- 工具集要 minimal viable，避免功能重叠导致 LLM 选择困难
- 从最简方案开始，仅在能证明改善时增加复杂度

#### 原则二：Transparency（透明性）

- 显式展示 Agent 的规划步骤（TodoWrite + thinking 事件）
- 写作过程可追溯（章节版本快照 + 修改历史）
- 上下文使用透明（agent:context_usage 事件已有）

#### 原则三：ACI（Agent-Computer Interface）

- 工具返回信息 token 高效
- 工具功能不重叠
- 工具自包含、健壮、用途清晰

#### 原则四：JIT 优于预加载（来自 Anthropic 上下文工程）

- 用工具按需检索，而非全量塞入上下文
- 维护轻量标识符（章节路径、角色 ID、伏笔 ID），运行时用工具动态加载
- 镜像人类认知——不背整本书，用索引按需检索

#### 原则五：不依赖堆上下文窗口

- 即使 Gemini 2M 也受 context rot 影响
- 必须 Compaction + 记忆管理 + JIT 检索三管齐下

### 4.3 不改造的范围（保留文档处理能力）

为遵循最小改动原则，**保留以下现有能力**：

- 16 个文件系统 Tool（write_text_file/read_file/list_directory 等）
- 4 个文档 Handler（docx/xlsx/pptx/pdf）
- Python Sidecar 文档处理能力
- VersionSnapshot 版本快照机制
- LlmRouter 多 Provider 适配
- 多语言国际化（i18n）
- 设置弹窗、工作区管理、网络监控等基础设施

文档处理模式与写作模式可共存，通过 TaskType 识别切换。

---

## 第五部分：改造总体架构

### 5.1 目标架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        前端 (React + TypeScript)                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │ 写作模式 UI  │  │ 文档模式 UI  │  │ Story Map 可视化面板     │  │
│  │ (新增)       │  │ (保留)       │  │ (角色/世界观/章节/伏笔)  │  │
│  └──────┬───────┘  └──────┬───────┘  └────────────┬─────────────┘  │
│         │                 │                       │                 │
│         └─────────────────┴───────────────────────┘                 │
│                              │ invoke/listen                       │
└──────────────────────────────┼──────────────────────────────────────┘
                               │
┌──────────────────────────────┼──────────────────────────────────────┐
│                        Rust 后端 (Tauri 2.x)                          │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                   AgentExecutor (扩展)                       │   │
│  │  ┌──────────┐  ┌─────────────┐  ┌──────────────────────┐  │   │
│  │  │ 主 Agent │─>│ Subagent    │  │ WritingStateMachine  │  │   │
│  │  │ 循环     │  │ 调度器(新) │  │ (新: 澄清→大纲→章节)  │  │   │
│  │  └────┬─────┘  └─────┬───────┘  └──────────┬───────────┘  │   │
│  │       │              │                      │              │   │
│  │       ▼              ▼                      ▼              │   │
│  │  ┌────────────────────────────────────────────────────┐  │   │
│  │  │           AgentContext (扩展)                       │  │   │
│  │  │  ┌──────────┐ ┌──────────┐ ┌─────────────────────┐│  │   │
│  │  │  │ Story Map│ │ Scratchpad│ │ Compaction Manager ││  │   │
│  │  │  │ (新)     │ │ (扩展)    │ │ (新)                ││  │   │
│  │  │  └──────────┘ └──────────┘ └─────────────────────┘│  │   │
│  │  └────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                Tool Registry (扩展)                         │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐│   │
│  │  │ 16 个现有 │ │ TodoWrite│ │ Task     │ │ Outline Mgr  ││   │
│  │  │ Tool     │ │ (新)     │ │ (新)     │ │ (新)         ││   │
│  │  ├──────────┤ ├──────────┤ ├──────────┤ ├──────────────┤│   │
│  │  │ Character│ │ World    │ │ Plot     │ │ Foreshadow   ││   │
│  │  │ Profile  │ │ Builder  │ │ Graph    │ │ Tracker      ││   │
│  │  │ (新)     │ │ (新)     │ │ (新)     │ │ (新)         ││   │
│  │  ├──────────┤ ├──────────┤ ├──────────┤ ├──────────────┤│   │
│  │  │ Chapter  │ │ Consist. │ │ Style    │ │ Search       ││   │
│  │  │ Manager  │ │ Checker  │ │ Analyzer │ │ Memory       ││   │
│  │  │ (新)     │ │ (新)     │ │ (新)     │ │ (新,P2)      ││   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────┘│   │
│  └─────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                Prompt System (扩展)                         │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐│   │
│  │  │ Writing  │ │ Task Type│ │ Document │ │ Prompt       ││   │
│  │  │ Design   │ │ Extend   │ │ Design   │ │ Loader       ││   │
│  │  │ (新)     │ │ (扩展)   │ │ (保留)   │ │ (集成)       ││   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────────┘│   │
│  └─────────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                Database (扩展)                              │   │
│  │  现有: sessions/messages/snapshots/templates/summaries     │   │
│  │  新增: characters/world_entries/plot_events/foreshadows/   │   │
│  │        chapters/outlines/scratchpad_entries                 │   │
│  └─────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
                               │ stdin/stdout JSON
┌──────────────────────────────┼──────────────────────────────────────┐
│                  Python Sidecar (扩展)                                │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────┐│
│  │ Word     │ │ Excel    │ │ PPT      │ │ PDF      │ │Markdown ││
│  │ (保留)   │ │ (保留)   │ │ (保留)   │ │ (保留)   │ │(扩展:章节)│
│  ├──────────┤ └──────────┘ └──────────┘ └──────────┘ └─────────┘│
│  │ Validator│ │ Story Map│ │ Style    │ │ Consist. │ │ Chroma  ││
│  │ (保留)   │ │ Parser   │ │ Analyzer │ │ Checker  │ │ RAG     ││
│  │          │ │ (新)     │ │ (新)     │ │ (新)     │ │ (新,P2) ││
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └─────────┘│
└──────────────────────────────────────────────────────────────────────┘
```

### 5.2 核心模块设计

#### 5.2.1 WritingStateMachine（写作状态机）

**位置**：`src-tauri/src/services/agent/writing_state_machine.rs`（新增）

**状态流转**：

```
[CLARIFYING] → [OUTLINING] → [OUTLINE_REVIEW] → [CHARACTER_BUILDING]
     ↓                                          ↓
 (用户澄清需求)                            [WORLDBUILDING]
     ↓                                          ↓
                                          [CHAPTER_GENERATING]
                                                  ↓
                                          [CONSISTENCY_CHECKING]
                                                  ↓
                                          [REFINING] (Self-Refine 循环)
                                                  ↓
                                          [FINALIZING] (输出文档)
```

**设计要点**：
- 每个状态对应不同的 System Prompt 子模板
- 状态切换由 Agent 显式调用 `transition_state` 工具触发
- 用户可在任何状态调用 `pause`/`resume`/`rollback`

#### 5.2.2 Subagent 调度器（Task 工具）

**位置**：`src-tauri/src/services/agent/subagent.rs`（新增）

**设计**：
- 复用现有 `AgentExecutor`，但传入独立的 `AgentContext`
- 主 Agent 通过 `task` 工具委派，传入 `task_type`（如 `chapter_generation`）+ `task_input`
- Subagent 执行完毕后返回**摘要结果**（而非完整对话历史）
- 支持 `parallel`（并行）和 `sequential`（串行）两种模式

**关键约束**：
- Subagent 上下文与主 Agent 完全隔离
- Subagent 可访问同一工作区，但消息历史独立
- Subagent 有独立的 max_iterations（默认 30）

#### 5.2.3 Story Map（故事地图）

**位置**：
- Rust 模型：`src-tauri/src/models/story_map.rs`（新增）
- Rust 仓库：`src-tauri/src/db/story_map_repo.rs`（新增）
- Python 解析器：`sidecar/handlers/story_map_parser.py`（新增）

**数据结构**：

```rust
// 角色档案（双层：稳定 + 变化）
pub struct CharacterProfile {
    pub id: String,
    pub session_id: String,
    pub name: String,
    // 稳定层
    pub age: Option<String>,
    pub appearance: Option<String>,
    pub personality: Option<String>,
    pub background: Option<String>,
    pub motivation: Option<String>,
    pub relationships: Vec<Relationship>,  // 关系网
    // 变化层（动态追踪）
    pub current_state: Option<String>,  // 心理状态
    pub current_goal: Option<String>,    // 阶段目标
    pub known_information: Vec<String>,  // 掌握信息
    pub abilities: Vec<String>,          // 能力变化
    pub first_appearance_chapter: u32,
    pub last_appearance_chapter: u32,
}

// 世界观条目
pub struct WorldEntry {
    pub id: String,
    pub session_id: String,
    pub category: WorldCategory,  // Geography/History/Magic/System/Society/Organization
    pub name: String,
    pub description: String,
    pub related_chapters: Vec<u32>,
    pub keywords: Vec<String>,  // 用于 Lorebook 式关键词触发
}

// 剧情事件
pub struct PlotEvent {
    pub id: String,
    pub session_id: String,
    pub chapter_number: u32,
    pub event_summary: String,
    pub involved_characters: Vec<String>,  // character_id 列表
    pub causality: Option<String>,  // 因果关系描述
    pub timestamp: Option<String>,  // 故事内时间
}

// 伏笔追踪
pub struct Foreshadow {
    pub id: String,
    pub session_id: String,
    pub setup_chapter: u32,        // 埋设章节
    pub setup_content: String,     // 埋设内容
    pub payoff_chapter: Option<u32>,  // 回收章节（None 表示未回收）
    pub payoff_content: Option<String>,
    pub status: ForeshadowStatus,  // Pending/Developed/PaidOff/Abandoned
    pub rotation_interval: Option<u32>,  // 轮换间隔（避免连续提及）
}

// 章节元数据
pub struct Chapter {
    pub id: String,
    pub session_id: String,
    pub chapter_number: u32,
    pub title: String,
    pub file_path: String,  // 工作区相对路径
    pub pov: Option<String>,  // 视角（限知/全知/第一人称）
    pub word_count: u32,
    pub status: ChapterStatus,  // Planned/Drafting/Reviewed/Final
    pub involved_characters: Vec<String>,
    pub summary: Option<String>,  // 章节摘要（用于后续检索）
}
```

**检索策略**（参考 Anthropic JIT + 混合检索）：

```rust
pub fn retrieve_relevant_context(
    &self,
    current_chapter: u32,
    current_scene: &str,
    max_tokens: usize,
) -> StoryMapContext {
    // 1. 优先级 1：当前章节大纲（最高优先级）
    let outline = self.get_chapter_outline(current_chapter);

    // 2. 优先级 2：上一章结尾（承接约束）
    let prev_ending = self.get_chapter_ending(current_chapter - 1);

    // 3. 优先级 3：本章涉及角色的最新状态
    let characters = self.get_character_states(&outline.involved_characters);

    // 4. 优先级 4：相关世界观条目（按 keywords 匹配）
    let world_entries = self.search_world_entries(&current_scene);

    // 5. 优先级 5：未回收伏笔（按相关性排序）
    let foreshadows = self.get_pending_foreshadows(current_chapter);

    // 6. 优先级 6：相关章节摘要（按语义相似度）
    let chapter_summaries = self.search_chapter_summaries(&current_scene, max_tokens);

    // 按 Token 预算裁剪
    StoryMapContext::truncate_to_budget(...)
}
```

#### 5.2.4 Compaction Manager（上下文压缩管理器）

**位置**：`src-tauri/src/services/agent/compaction.rs`（新增）

**触发条件**：
- 上下文使用量超过 80% 时自动触发
- 章节切换时触发（每完成一章压缩前文）
- 用户手动调用 `compact_context` 工具

**压缩流程**（参考 Anthropic Compaction）：

```rust
pub async fn compact_context(&self, ctx: &mut AgentContext) -> Result<()> {
    // 1. 识别需要压缩的早期消息范围
    let compactable_range = self.identify_compactable_range(ctx);

    // 2. 调用 LLM 生成高保真结构化摘要
    let summary = self.llm_compact_summary(&ctx.messages[compactable_range]).await?;

    // 3. 用摘要替换早期消息
    ctx.replace_messages_with_summary(compactable_range, summary);

    // 4. 持久化摘要到 StoryMap（章节摘要）
    self.persist_chapter_summary(summary).await?;

    // 5. 重置 persisted_count
    ctx.reset_persisted_count();
}
```

**摘要结构**（高保真，非简单截断）：

```rust
pub struct CompactedSummary {
    pub character_states: Vec<CharacterStateSummary>,  // 各角色当前状态
    pub plot_progress: Vec<PlotEventSummary>,            // 已发生的关键事件
    pub foreshadow_status: Vec<ForeshadowStatusSummary>, // 伏笔状态
    pub style_notes: Vec<String>,                        // 文风备忘
    pub unresolved_threads: Vec<String>,                 // 未解决的线索
}
```

#### 5.2.5 写作专用 System Prompt 架构

**位置**：`src-tauri/src/services/agent/prompts/writing_design.rs`（新增）

**分层设计**（基于现有 7 层架构扩展）：

```
[全局 System Prompt]
├── Layer 0: 写作身份（小说家/学术作者/散文家/评论家）
├── Layer 1: 写作规则（必须遵守 + 禁止行为）
├── Layer 2: 上下文（工作区/作者信息/写作状态机当前状态）
├── Layer 3: 工具策略（Story Map 检索/章节生成/一致性检查）
├── Layer 4: 防幻觉（信息诚实 + 设定遵循）
├── Layer 5: 错误处理
├── Layer 6: 文体规范（按文体路由）
│   ├── NOVEL_DESIGN_GUIDE（三幕剧/英雄之旅/角色弧光/伏笔技巧）
│   ├── PAPER_DESIGN_GUIDE（IMRAD 结构/引用规范/图表规范）
│   ├── NARRATIVE_DESIGN_GUIDE（时间顺序/人物描写/场景构建）
│   ├── ARGUMENTATIVE_DESIGN_GUIDE（论点/论据/反驳/结论）
│   └── SCRIPT_DESIGN_GUIDE（剧本格式/对白/场景指示）
└── Layer 7: 写作示例（按文体 + 任务阶段路由）
```

### 5.3 数据流

#### 5.3.1 长篇小说生成流程

```
用户："写一本 30 万字的科幻小说，主角是宇航员李明"

1. [CLARIFYING 状态]
   Agent 调用 TodoWrite 创建任务清单
   Agent 通过 ask_user 工具澄清：
     - 题材细分（硬科幻/软科幻/赛博朋克？）
     - 文风偏好（白描/华丽？第一人称/第三人称？）
     - 目标读者（青少年/成人？）
   用户回复后 → 切换到 OUTLINING

2. [OUTLINING 状态]
   Agent 调用 outline_manager 工具：
     - 生成全书大纲（卷/章/节三层）
     - 使用 Chain of Density 5 轮加密
   Agent 调用 character_profile 工具：
     - 创建主角李明的稳定层档案
     - 创建配角档案
   Agent 调用 worldbuilding_manager 工具：
     - 构建世界观（星际地理/科技体系/社会规则）
   Agent 调用 foreshadow_tracker 工具：
     - 规划关键伏笔分布
   → 切换到 OUTLINE_REVIEW

3. [OUTLINE_REVIEW 状态]
   Agent 调用 Subagent (task_type=outline_review):
     评审大纲完整性/角色弧光/伏笔分布
   Subagent 返回评审报告
   Agent 根据反馈调整大纲
   → 切换到 CHAPTER_GENERATING

4. [CHAPTER_GENERATING 状态] (循环 30 次)
   每章生成流程：
   a. Agent 调用 story_map 工具检索相关上下文（JIT）
   b. Agent 调用 Subagent (task_type=chapter_generation):
      - 传入：章节大纲 + 涉及角色 + 世界观 + 伏笔 + 上一章结尾
      - Subagent 在独立上下文中生成章节正文
      - 使用 AgentWrite 管道：计划段落 → 串行书写
      - 返回：章节正文摘要
   c. Agent 调用 write_text_file 写入章节文件
   d. Agent 调用 character_profile 更新角色状态（变化层）
   e. Agent 调用 foreshadow_tracker 更新伏笔状态
   f. Agent 调用 consistency_checker 检查一致性
   g. 若 Compaction 触发条件满足 → 调用 compact_context
   h. Agent 调用 TodoWrite 标记章节完成

5. [CONSISTENCY_CHECKING 状态]
   Agent 调用 Subagent (task_type=consistency_review):
     全书一致性检查（角色/时间线/伏笔/设定）
   Subagent 返回问题清单
   Agent 调用 TodoWrite 创建修订任务

6. [REFINING 状态]
   对每个问题章节：
   a. Agent 调用 Subagent (task_type=chapter_refine):
      - 传入：原章节 + 问题清单 + 修改建议
      - Self-Refine 循环：生成 → 自评 → 修改（最多 3 轮）
      - 返回：修订后章节
   b. Agent 调用 write_text_file 覆盖章节文件

7. [FINALIZING 状态]
   Agent 调用 markdown_handler.convert 将 Markdown 转换为 docx
   Agent 调用 validator_handler 检查文档格式
   Agent 发射 agent:done 事件
```

---

## 第六部分：分阶段改造任务

### Phase 0：基础准备（1-2 天）

#### 任务 0.1：扩展 TaskType 枚举

**文件**：[src-tauri/src/services/agent/prompts/task_type.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/task_type.rs)

**改动**：
- 新增枚举变体：`Novel, Novella, ShortStory, ResearchPaper, ArgumentativeEssay, NarrativeEssay, Script`
- 扩展 `from_user_message` 关键词识别：
  - Novel：小说/长篇/科幻/奇幻/武侠/言情
  - ResearchPaper：论文/科研/学术/IMRAD
  - ArgumentativeEssay：议论文/辩论/论证
  - NarrativeEssay：记叙文/叙事/散文
- 扩展 `required_guide_types` 支持 Novel 等类型返回写作规范

#### 任务 0.2：参数化 System Prompt 身份层

**文件**：[context.rs:834-855](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs)

**改动**：
- `layer_identity()` 改为 `layer_identity(task_type: &TaskType)`，按任务类型返回不同身份
- 写作类型返回"小说家/学术作者/散文家"身份
- 文档类型保持现有"AI 文档处理专家"身份

#### 任务 0.3：新增写作设计指南

**文件**：`src-tauri/src/services/agent/prompts/writing_design.rs`（新增）

**内容**：
- `NOVEL_DESIGN_GUIDE`：三幕剧结构、英雄之旅、角色弧光、伏笔技巧、章节节奏、人称视角
- `PAPER_DESIGN_GUIDE`：IMRAD 结构、引用规范、图表规范、学术语言
- `NARRATIVE_DESIGN_GUIDE`：时间顺序、人物描写、场景构建、五感描写
- `ARGUMENTATIVE_DESIGN_GUIDE`：论点结构、论据类型、反驳技巧、逻辑谬误
- `get_writing_guide_by_type(task_type: &TaskType) -> Option<&'static str>`

#### 任务 0.4：调整 Executor 关键常量

**文件**：[executor.rs:20-52](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/executor.rs)

**改动**：
- `MAX_TOOL_RESULT_CHARS`：从 6000 → 12000（写作场景需要更多上下文）
- `MAX_TOKENS_CEILING`：从 131072 → 262144（写作场景需要更长响应）
- `max_iterations` 默认值：从 100 → 500（长篇创作需要更多迭代）
- 这些常量可按 TaskType 动态调整（写作类型使用更大值）

---

### Phase 1（P0）：写作型 Agent 骨架（5-7 天）

#### 任务 1.1：新增 TodoWrite 工具

**文件**：`src-tauri/src/services/tool/builtin/todo_write.rs`（新增）

**功能**：
- 任务清单 CRUD（content/priority/status/id）
- 状态：pending/in_progress/completed
- 优先级：high/medium/low
- 持久化到 SQLite（新表 `todos`）

**Schema**：
```json
{
  "name": "todo_write",
  "parameters": {
    "action": {"type": "string", "enum": ["create", "update", "list", "delete"]},
    "todo": {"type": "object", "properties": {"id", "content", "priority", "status"}}
  }
}
```

#### 任务 1.2：新增 Subagent（Task）工具

**文件**：`src-tauri/src/services/agent/subagent.rs`（新增）+ `src-tauri/src/services/tool/builtin/task.rs`（新增）

**核心设计**：
- `Task` 工具接受 `task_type` + `task_input` + `preferred_provider`（可选）
- 内部启动独立 `AgentExecutor` 实例，传入新建的 `AgentContext`
- Subagent 拥有独立的 session_id、消息历史、Scratchpad
- Subagent 完成后返回结构化摘要（非完整对话历史）
- 支持 `parallel`（并行）和 `sequential`（串行）模式

**支持的 task_type**：
- `outline_generation`：大纲生成
- `outline_review`：大纲评审
- `chapter_generation`：章节生成（核心）
- `chapter_refine`：章节修订
- `consistency_review`：一致性评审
- `style_analysis`：风格分析

**关键约束**：
- Subagent 上下文与主 Agent 完全隔离
- Subagent 可访问同一工作区文件，但消息历史独立
- Subagent 默认 max_iterations = 30（可配置）
- Subagent 不可再创建 Subagent（防递归爆炸）

#### 任务 1.3：新增 outline_manager 工具

**文件**：`src-tauri/src/services/tool/builtin/outline_manager.rs`（新增）

**功能**：
- 多层大纲管理（全书→卷→章→节）
- 节点 CRUD：create/update/delete/move
- 节点关系：因果/并列/递进
- 持久化到 SQLite（新表 `outlines`）

**Schema**：
```json
{
  "name": "outline_manager",
  "parameters": {
    "action": {"enum": ["create_node", "update_node", "delete_node", "move_node", "get_outline", "get_subtree"]},
    "node": {"properties": {"id", "parent_id", "level", "title", "summary", "word_target", "order"}}
  }
}
```

#### 任务 1.4：新增 character_profile 工具

**文件**：`src-tauri/src/services/tool/builtin/character_profile.rs`（新增）

**功能**：
- 角色档案 CRUD（双层：稳定 + 变化）
- 稳定层：姓名/年龄/外貌/性格/背景/动机/关系网
- 变化层：当前状态/阶段目标/掌握信息/能力变化
- 持久化到 SQLite（新表 `characters`）

#### 任务 1.5：扩展 Scratchpad 为多类型

**文件**：[builtin.rs:3186-3379](file:///d:/DeskTop/DocAgent/src-tauri/src/services/tool/builtin.rs) + [models/tool.rs:23-30](file:///d:/DeskTop/DocAgent/src-tauri/src/models/tool.rs)

**改动**：
- 单条笔记限制 500 → 2000 字符
- 新增 `category` 字段：progress/character/world/foreshadow/plot/style/todo
- 新增 `update` 和 `delete` 操作（按 ID 或索引）
- 摘要格式按 category 分组展示

#### 任务 1.6：调整 Token 预算与滑动窗口

**文件**：[token_budget.rs:46-57, 132-140](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/prompts/token_budget.rs)

**改动**：
- 写作类型（Novel/Novella 等）的 response 配额：25% → 40%
- 写作类型的滑动窗口上限：10 → 50（长上下文模型时）
- `keep_recent_rounds`：2 → 5（写作场景）

#### 任务 1.7：增强摘要占位为 LLM 摘要

**文件**：[context.rs:578-664](file:///d:/DeskTop/DocAgent/src-tauri/src/services/agent/context.rs)

**改动**：
- `compress_history_if_needed` 改为异步调用 LLM 生成结构化摘要
- 摘要包含：角色状态、剧情进度、伏笔状态、文风备忘
- 持久化摘要到 `chapter_summaries` 表（新表）

---

### Phase 2（P1）：结构化创作记忆（7-10 天）

#### 任务 2.1：新增数据库表

**文件**：[src-tauri/src/db/init.rs](file:///d:/DeskTop/DocAgent/src-tauri/src/db/init.rs)

**新增表**：
```sql
-- 角色档案
CREATE TABLE characters (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    name TEXT NOT NULL,
    age TEXT, appearance TEXT, personality TEXT, background TEXT, motivation TEXT,
    relationships TEXT,  -- JSON 数组
    current_state TEXT, current_goal TEXT,
    known_information TEXT,  -- JSON 数组
    abilities TEXT,  -- JSON 数组
    first_appearance_chapter INTEGER,
    last_appearance_chapter INTEGER,
    created_at TEXT, updated_at TEXT
);

-- 世界观条目
CREATE TABLE world_entries (...);

-- 剧情事件
CREATE TABLE plot_events (...);

-- 伏笔
CREATE TABLE foreshadows (...);

-- 章节
CREATE TABLE chapters (...);

-- 大纲节点
CREATE TABLE outlines (...);

-- 章节摘要
CREATE TABLE chapter_summaries (...);

-- Scratchpad 持久化
CREATE TABLE scratchpad_entries (...);

-- Todo 任务
CREATE TABLE todos (...);
```

#### 任务 2.2：新增 worldbuilding_manager 工具

**文件**：`src-tauri/src/services/tool/builtin/worldbuilding_manager.rs`（新增）

**功能**：
- 世界观条目 CRUD（地理/历史/魔法/社会/组织）
- 关键词标签（用于 Lorebook 式触发）
- 关联章节追踪

#### 任务 2.3：新增 plot_graph 工具

**文件**：`src-tauri/src/services/tool/builtin/plot_graph.rs`（新增）

**功能**：
- 剧情事件 CRUD
- 因果关系图（节点 + 边）
- 按章节范围查询事件

#### 任务 2.4：新增 foreshadow_tracker 工具

**文件**：`src-tauri/src/services/tool/builtin/foreshadow_tracker.rs`（新增）

**功能**：
- 伏笔生命周期管理（埋设→推进→回收→归档）
- 状态：Pending/Developed/PaidOff/Abandoned
- 轮换间隔控制（避免连续 N 章都提）
- 未回收伏笔清单查询

#### 任务 2.5：新增 chapter_manager 工具

**文件**：`src-tauri/src/services/tool/builtin/chapter_manager.rs`（新增）

**功能**：
- 章节元数据 CRUD
- 章节状态机：Planned/Drafting/Reviewed/Final
- 章节排序、字数统计
- 章节摘要生成与存储

#### 任务 2.6：重新启用跨会话摘要注入（可配置）

**文件**：[agent.rs:862-864](file:///d:/DeskTop/DocAgent/src-tauri/src/commands/agent.rs)

**改动**：
- 新增配置项 `cross_session_summary_injection`（默认 false，写作模式可启用）
- 写作模式下，新会话启动时读取同工作区历史摘要注入
- 扩展 `ContextSessionSummary` 增加 `chapter_summary` 字段（结构化摘要）

#### 任务 2.7：Scratchpad 持久化

**文件**：[builtin.rs:3186-3379](file:///d:/DeskTop/DocAgent/src-tauri/src/services/tool/builtin.rs) + 新增 `src-tauri/src/db/scratchpad_repo.rs`

**改动**：
- 会话结束时将内存态 Scratchpad 落盘到 `scratchpad_entries` 表
- 新会话启动时从数据库恢复 Scratchpad
- 保持 SharedScratchpadStates 的内存态用于高性能读写，数据库作为持久化层

#### 任务 2.8：新增 consistency_checker 工具

**文件**：`src-tauri/src/services/tool/builtin/consistency_checker.rs`（新增）

**功能**：
- 角色一致性检查（姓名/年龄/外貌/性格前后矛盾）
- 时间线冲突检测
- 伏笔状态检查（未回收/重复埋设）
- 世界观设定违反检测

---

### Phase 3（P2）：长文本生成引擎（10-15 天）

#### 任务 3.1：实现 Compaction Manager

**文件**：`src-tauri/src/services/agent/compaction.rs`（新增）

**功能**：
- 上下文使用量监控（超过 80% 自动触发）
- 调用 LLM 生成高保真结构化摘要
- 用摘要替换早期消息
- 持久化摘要到 `chapter_summaries` 表

#### 任务 3.2：实现 AgentWrite 分段串行生成

**文件**：`src-tauri/src/services/agent/subagent.rs`（扩展）

**功能**：
- 在 `chapter_generation` Subagent 中实现 AgentWrite 管道：
  - 计划阶段：生成段落大纲（每段 200-1000 字）
  - 串行书写：生成第 n 段时输入前 n-1 段历史
- 突破单次 max_tokens 限制，支持 20000+ 字章节生成

#### 任务 3.3：实现 Chain of Density 大纲精炼

**文件**：`src-tauri/src/services/agent/subagent.rs`（扩展）

**功能**：
- 大纲生成 Subagent 支持 5 轮密度链精炼
- 初始粗略大纲 → 识别缺失实体 → 增加密度重写 → 重复 5 次
- 输出高密度详纲

#### 任务 3.4：实现 Self-Refine 评审循环

**文件**：`src-tauri/src/services/agent/subagent.rs`（扩展）

**功能**：
- `chapter_refine` Subagent 实现 Self-Refine 循环：
  - 生成初稿 → 自评（人物/情节/文风/张力） → 修改 → 重复（最多 3 轮）
- "只升不降"接受策略（参考 PaperOrchestra）

#### 任务 3.5：新增 style_analyzer 工具

**文件**：`src-tauri/src/services/tool/builtin/style_analyzer.rs`（新增）+ `sidecar/handlers/style_analyzer.py`（新增）

**功能**：
- 句长分布分析
- 词汇丰富度统计
- 对话/叙述比例
- 节奏分析（快/慢/中）
- 风格样本学习（提取参考文本的风格特征）

#### 任务 3.6：新增 search_memory 工具（向量检索）

**文件**：`src-tauri/src/services/tool/builtin/search_memory.rs`（新增）+ `sidecar/handlers/vector_store.py`（新增）

**功能**：
- 集成 Chroma 向量数据库（嵌入式，无需独立服务）
- 对已生成章节、角色档案、风格样本做 embedding
- 支持语义检索（"主角被同伴怀疑的情节"）
- JIT 策略：按需检索，非全量注入

#### 任务 3.7：扩展 Python Sidecar 支持 Markdown 章节操作

**文件**：[sidecar/handlers/markdown_handler.py](file:///d:/DeskTop/DocAgent/sidecar/handlers/markdown_handler.py)

**改动**：
- 新增 `get_chapter` 操作：按章节号切片返回
- 新增 `get_chapter_outline` 操作：提取章节标题层级
- 新增 `merge_chapters` 操作：合并多章节为完整文档
- 新增 `front_matter` 操作：管理 front-matter 元数据

---

### Phase 4（P3）：高级能力（可选，10-15 天）

#### 任务 4.1：多 Agent 角色化协作（CrewAI 式）

**位置**：`src-tauri/src/services/agent/roles/`（新增目录）

**角色设计**：
- `ChiefEditor`（主编）：负责整体规划、章节分配、最终审校
- `Outliner`（大纲师）：专注大纲生成与精炼
- `CharacterDesigner`（角色设计师）：专注角色档案创建
- `WorldBuilder`（世界观架构师）：专注世界观构建
- `ChapterWriter`（章节写手）：专注章节正文生成
- `ConsistencyReviewer`（一致性审校员）：专注一致性检查
- `StyleEditor`（文风编辑）：专注文风统一与润色

#### 任务 4.2：知识图谱检索（仅动感叙事）

**位置**：`sidecar/handlers/knowledge_graph.py`（新增）

**适用场景**：
- 动感叙事（动作/探险/科幻）：KG 显著正面效果
- 内省叙事（心理恐怖/浪漫戏剧）：**不使用 KG**（负面效果）

**功能**：
- 多跳关联查询（"和林远有关系的所有角色"）
- 关系强度排序
- 按关系类型过滤

#### 任务 4.3：写作专用 Skill 系统

**位置**：`src-tauri/src/services/agent/skills/`（新增目录）

**Skill 设计**（参考 Claude Code superpowers）：
- `brainstorming_writing`：写作需求澄清
- `outline_planning`：大纲规划
- `character_creation`：角色创建
- `chapter_drafting`：章节起草
- `chapter_refining`：章节修订
- `final_polishing`：最终润色

#### 任务 4.4：前端写作模式 UI

**位置**：`src/components/writing/`（新增目录）

**UI 设计**：
- 写作模式入口（切换文档模式/写作模式）
- Story Map 可视化面板（角色/世界观/章节/伏笔树状图）
- 章节进度面板（已完成/进行中/待办）
- 写作状态机可视化（当前状态 + 流转历史）
- 角色档案编辑器
- 伏笔追踪面板

---

## 第七部分：最终决策记录

> 以下决策已于 2026-07-08 由用户确认，作为后续 writing-plans 阶段的设计输入。

### 7.1 文档处理模式

**最终决策**：**文档为辅**（保留文档处理为子能力，写作模式为主）
- 写作模式作为应用主入口
- 文档处理能力保留，作为辅助能力通过 TaskType 自动切换
- 用户输入写作类请求时进入写作模式，输入文档处理请求时进入文档模式

### 7.2 支持的写作类型

**最终决策**：**全场景**
- 长篇小说（10-50 万字）
- 中短篇小说（1-10 万字）
- 科研论文（5000-20000 字，IMRAD 结构）
- 记叙文（1000-5000 字）
- 议论文（1000-5000 字）
- 剧本（含对白、场景指示）

### 7.3 长文本生成策略

**最终决策**：**混合模式**（关键节点确认，其他自动）
- 自动阶段：澄清→大纲生成→角色构建→世界观构建→章节生成
- **需用户确认的关键节点**：
  1. 大纲评审通过后（OUTLINE_REVIEW → CHAPTER_GENERATING）
  2. 一致性检查后的问题修订方案（CONSISTENCY_CHECKING → REFINING）
  3. 最终定稿前（REFINING → FINALIZING）
- 其他状态切换由 Agent 自主决策

### 7.4 向量检索引入时机

**最终决策**：**Phase 3 才引入**
- Phase 1/2 依赖 Scratchpad + LLM 高保真摘要 + Compaction 应对
- Phase 3 才集成 Chroma 向量数据库，作为长篇（>20 万字）增强能力
- Phase 3 前用 SQLite 全文检索 + 关键词匹配作为降级方案

### 7.5 持久化方案

**最终决策**：**复用 SQLite**
- 与现有 rusqlite 架构保持一致
- 新增 9 张表（characters/world_entries/plot_events/foreshadows/chapters/outlines/chapter_summaries/scratchpad_entries/todos）
- 不引入额外的 JSON 文件存储或向量数据库（Chroma 除外，仅 Phase 3 引入）

### 7.6 LLM 模型选择策略

**最终决策**：**单一模型**（用户配置的主 Provider）
- 所有写作阶段（大纲/角色/章节/润色）使用同一 Provider
- 简化架构，避免多 Provider 协调复杂度
- 用户可在设置中切换主 Provider（如从 DeepSeek 切到 Claude）
- 不实现按任务类型路由或多模型投票

### 7.7 多 Agent 角色化时机

**最终决策**：**Phase 4 才考虑**
- Phase 1-3 保持单 Agent + Subagent 架构
- Subagent 用于章节生成、一致性评审、风格分析等独立任务
- Phase 4 才考虑引入 CrewAI 式多角色（主编/写手/编辑等）
- 若 Phase 1-3 验证 Subagent 架构已足够，可跳过 Phase 4

### 7.8 决策对原计划的影响调整

基于以上决策，对原计划做以下调整：

1. **任务 4.1（多 Agent 角色化）**：从 Phase 4 改为"可选实施"，且不依赖按任务类型路由模型
2. **任务 0.2（参数化身份层）**：写作模式作为默认身份，文档处理为辅助身份
3. **任务 1.2（Subagent 设计）**：不实现按任务类型路由 Provider，统一使用主 Provider
4. **任务 5.3.1（生成流程）**：在 OUTLINE_REVIEW → CHAPTER_GENERATING、CONSISTENCY_CHECKING → REFINING、REFINING → FINALIZING 三个节点插入用户确认 gate
5. **任务 2.6（跨会话摘要注入）**：因用户明确要求"每个会话应该完全独立"，写作模式下也保持禁用，依赖 Scratchpad 持久化 + Story Map 持久化来支撑跨会话续写

---

## 第八部分：风险评估与验证策略

### 8.1 风险评估

| 风险 | 等级 | 影响 | 缓解措施 |
|------|------|------|---------|
| **上下文爆炸** | 高 | 长篇创作时 LLM 上下文超限 | Subagent 隔离 + Compaction + JIT 检索 |
| **长程一致性丢失** | 高 | 人物/伏笔/世界观前后矛盾 | Story Map 持久化 + consistency_checker |
| **Subagent 递归爆炸** | 中 | Subagent 创建 Subagent 导致无限递归 | 禁止 Subagent 创建 Subagent |
| **LLM API 成本** | 中 | 长篇创作需大量 LLM 调用 | 按任务类型选择模型（reasoning vs 写作） |
| **用户中断风险** | 中 | 长任务执行中用户关闭应用 | 增量持久化 + Scratchpad 落盘 |
| **Chroma 集成复杂度** | 低 | Python Sidecar 集成难度 | Phase 3 才引入，先用 Scratchpad 应对 |
| **现有架构破坏** | 中 | 改动可能影响文档处理模式 | 双模式共存，TaskType 隔离 |
| **System Prompt 膨胀** | 中 | 写作指南增加 Token 占用 | 按 TaskType 动态注入，非全量加载 |

### 8.2 验证策略

#### 8.2.1 单元测试

每个新增 Tool/Handler 必须有单元测试：
- 工具 CRUD 操作测试
- 数据库仓库测试
- 状态机流转测试

#### 8.2.2 集成测试

- 短篇小说生成（1 万字）端到端测试
- 中篇小说生成（5 万字）端到端测试
- 跨会话续写测试（关闭重开后续写第 51 章）

#### 8.2.3 一致性测试

- 角色一致性检查（生成 10 章后检查角色档案）
- 伏笔回收率检查（埋设 10 个伏笔，检查回收率）
- 章节衔接测试（每章结尾与下章开头是否衔接）

#### 8.2.4 性能测试

- 上下文使用量监控（确保不超过 80%）
- Compaction 触发频率与效果
- Subagent 上下文隔离验证

#### 8.2.5 用户体验测试

- 一句话生成短篇小说（5 分钟内完成）
- 一句话生成中篇小说（30 分钟内完成）
- 用户中断后恢复测试

---

## 第九部分：里程碑与交付物

### 9.1 里程碑

| 里程碑 | 内容 | 交付物 |
|--------|------|--------|
| **M1**：写作型 Agent 骨架 | Phase 0 + Phase 1 | 短篇小说（1 万字）端到端生成 |
| **M2**：结构化创作记忆 | Phase 2 | 中篇小说（5 万字）跨会话续写 |
| **M3**：长文本生成引擎 | Phase 3 | 长篇小说（30 万字）连贯创作 |
| **M4**：高级能力 | Phase 4 | 多 Agent 协作 + 前端可视化 |

### 9.2 各阶段交付物

#### Phase 0 交付物
- 扩展的 TaskType 枚举
- 参数化的 System Prompt 身份层
- 新增的 writing_design.rs（5 类写作指南）
- 调整后的 Executor 常量

#### Phase 1 交付物
- TodoWrite 工具
- Subagent（Task）工具
- outline_manager 工具
- character_profile 工具
- 扩展的 Scratchpad（多类型 + 持久化）
- 调整后的 Token 预算与滑动窗口
- LLM 高保真摘要

#### Phase 2 交付物
- 9 张新数据库表
- worldbuilding_manager 工具
- plot_graph 工具
- foreshadow_tracker 工具
- chapter_manager 工具
- consistency_checker 工具
- 跨会话摘要注入（可配置）
- Scratchpad 持久化

#### Phase 3 交付物
- Compaction Manager
- AgentWrite 分段串行生成
- Chain of Density 大纲精炼
- Self-Refine 评审循环
- style_analyzer 工具
- search_memory 工具（Chroma 集成）
- 扩展的 Markdown Handler（章节操作）

#### Phase 4 交付物
- 多 Agent 角色化（7 个角色）
- 知识图谱检索（动感叙事）
- 写作专用 Skill 系统
- 前端写作模式 UI

---

## 第十部分：附录

### 10.1 关键代码位置索引

| 模块 | 文件 | 关键行号 |
|------|------|---------|
| 系统提示词 7 层组装 | context.rs | 791-831 |
| 身份层 | context.rs | 834-855 |
| TaskType 枚举 | task_type.rs | 8-26 |
| 任务类型识别 | task_type.rs | 31-99 |
| Token 预算分配 | token_budget.rs | 46-57 |
| 滑动窗口策略 | token_budget.rs | 132-140 |
| 消息列表组装 | context.rs | 511-574 |
| 对话历史压缩 | context.rs | 578-664 |
| Scratchpad 摘要注入 | context.rs | 561-571 |
| ScratchpadTool 实现 | builtin.rs | 3186-3379 |
| Scratchpad 单条上限 | builtin.rs | 3264 |
| Tool 注册 | builtin.rs | 39-77 |
| Handler 注册 | builtin.rs | 478-490 |
| 执行循环主入口 | executor.rs | 432-1426 |
| 最大迭代次数 | executor.rs | 147 |
| 工具结果截断 | executor.rs | 52 |
| 工具结果截断逻辑 | executor.rs | 1244-1320 |
| 增量持久化 | executor.rs | 291-300 |
| 截断重试 | executor.rs | 822-1029 |
| Subagent 注入点 | executor.rs | 1156-1159 |
| 跨会话摘要禁用 | agent.rs | 862-864 |
| 情景记忆提取 | context.rs | 701-776 |
| 用户偏好提取 | agent.rs | 533-578 |
| 长上下文预设 | context_presets.rs | 24-158 |

### 10.2 调研来源清单

#### Anthropic 官方核心（一手原文）
- Effective context engineering for AI agents：https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- Building effective agents：https://www.anthropic.com/engineering/building-effective-agents
- Writing tools for AI agents：https://www.anthropic.com/engineering/writing-tools-for-agents

#### 写作 Agent 工具
- Sudowrite：https://www.sudowrite.com/
- NovelAI：https://novelai.net/
- NovelCrafter：https://www.novelcrafter.com/
- Jasper：https://www.jasper.ai/

#### 学术 Agent
- FARS：上海日行迹智能科技
- PaperOrchestra：https://arxiv.org/abs/2604.05018
- SciSpace：https://scispace.com/

#### 多 Agent 协作论文
- Agents' Room（ICLR 2025）：https://arxiv.org/abs/2410.02603
- LongWriter（清华）：https://github.com/THUDM/LongWriter
- MemGPT（ICLR 2024）：https://github.com/letta-ai/letta
- Generative Agents（UIST 2023）

#### 知名通用 Agent
- Claude Code：https://www.anthropic.com/claude-code
- Aider：https://github.com/Aider-AI/aider
- OpenHands：https://github.com/All-Hands-AI/OpenHands
- OpenCode：https://opencode.ai
- Codex CLI：https://github.com/openai/codex
- GPT Engineer：https://github.com/AntonOsika/gpt-engineer

#### 多 Agent 框架
- LangGraph：https://github.com/langchain-ai/langgraph
- CrewAI：https://github.com/crewAIInc/crewAI
- AutoGen：https://github.com/microsoft/autogen

#### 写作专用提示词技术
- Self-Refine：https://selfrefine.info/
- Chain of Density：https://arxiv.org/abs/2309.04269
- Lost in Stories：长篇一致性研究

#### 向量数据库
- Chroma：https://www.trychroma.com/
- Qdrant：https://qdrant.tech/
- LanceDB：https://lancedb.github.io/lancedb/

---

## 总结

本改造计划基于对 DocAgent 项目源码的全面深度分析（7 个核心文件、约 4800 行 Rust 代码）和对前沿写作 Agent 工具（Sudowrite/NovelAI/NovelCrafter/PaperOrchestra/Agents' Room）及知名通用 Agent（Claude Code/Aider/OpenHands/OpenCode/Codex）的联网调研，制定了分四阶段的改造路径：

**核心改造方向**：
1. **P0**：扩展 TaskType + 参数化身份层 + 新增 TodoWrite/Subagent/outline_manager/character_profile 工具 + 调整 Token 预算
2. **P1**：新增 9 张数据库表 + 4 个写作专用工具 + Scratchpad 持久化 + 跨会话摘要注入 + LLM 高保真摘要
3. **P2**：Compaction Manager + AgentWrite 分段生成 + Chain of Density + Self-Refine + Chroma 向量检索
4. **P3**：多 Agent 角色化 + 知识图谱 + 写作专用 Skill 系统 + 前端写作模式 UI

**关键设计原则**：
- 遵循 Anthropic 三原则（Simplicity/Transparency/ACI）
- JIT 优于预加载，不依赖堆上下文窗口
- 在现有架构上叠加写作层，而非重写
- 保持 Rust 架构纯净，不引入 Python 框架依赖

**待用户确认的决策点**（共 7 项）：
1. 文档处理模式是否保留（推荐双模式共存）
2. 支持哪些写作类型（推荐全场景）
3. 持久化方案（推荐 SQLite）
4. 模型选择策略（推荐按任务类型路由）
5. 长文本生成策略（推荐一句话全自动）
6. 向量检索引入时机（推荐 Phase 3）
7. 多 Agent 角色化时机（推荐 Phase 4）

待用户确认关键决策点后，将进入 writing-plans 阶段，产出 TDD 化的可执行任务清单（每个任务 2-5 分钟，含完整测试代码和实现代码）。

---

**文档结束**
