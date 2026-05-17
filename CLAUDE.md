# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

DocAgent 是一款 AI 文档处理桌面应用，通过自然语言驱动 Agent 完成 Word、Excel、PDF、PPT、Markdown 等文档的生成、修改、格式转换等操作。技术栈：Tauri 2 + React 19 + TypeScript 5 + Zustand 5 + Tailwind CSS 4。

## 开发阶段（Phase 1 MVP 中期）

当前各模块完成度：

| 模块 | 完成度 | 代码量 | 说明 |
|------|--------|--------|------|
| 前端 UI 组件 | 90% | 约 3,689 行 | 组件、Store、事件封装全部完成，待与后端联调 |
| Rust 后端 | 65% | 约 3,688 行 | 数据库、配置、模型、事件系统完成，命令逻辑待完善 |
| Python Sidecar | 85% | 约 1,217 行 | 所有文档处理器已实现，待与后端集成 |
| 共享类型 | 20% | 10 行 | 仅定义了 NodeType 和 ExecutionStatus |
| 设计文档 | 100% | 约 9,594 行 | PRD、技术架构、组件设计、数据库设计等齐全 |

### 各模块详细状态

#### 前端 UI 组件（90% 完成）

| 子模块 | 状态 | 说明 |
|--------|------|------|
| 布局组件 | 完成 | TopBar、MainLayout、Sidebar、MainArea、InputArea |
| 工作流节点 | 完成 | WorkflowTimeline + 7 种节点组件，支持展开/折叠 |
| 侧边栏区块 | 完成 | AgentInfo、FileTree、Todo、Token 四个区块 |
| 设置对话框 | 完成 | SettingsDialog + 5 个标签页 |
| 预览面板 | 完成 | PreviewOverlay 支持文档预览和差异对比 |
| 状态管理 | 完成 | 6 个 Zustand Store 全部实现 |
| 事件监听 | 完成 | 完整的 Agent 事件监听封装（event.ts） |
| Tauri 命令封装 | 完成 | 所有命令的 TypeScript 封装（tauri.ts） |

#### Rust 后端（65% 完成）

| 子模块 | 状态 | 说明 |
|--------|------|------|
| 数据库层 | 完成 | SQLite 封装、5 张表、CRUD 操作全部实现 |
| 配置管理 | 完成 | LLM 配置、应用设置、工作区配置全部实现 |
| 模型定义 | 完成 | 所有数据模型已定义 |
| 事件系统 | 完成 | AgentEmitter 和事件类型全部实现 |
| LLM 服务 | 80% | OpenAI 适配器完整实现，Claude/Gemini 待实现 |
| Agent 执行器 | 70% | Tool Calling 循环核心逻辑已实现 |
| Skill 注册表 | 60% | 注册表框架已实现，内置 Skills 待完善 |
| Tauri 命令 | 40% | 命令已注册，多数为桩代码待实现 |

#### Python Sidecar（85% 完成）

| 处理器 | 状态 | 功能 |
|--------|------|------|
| Word 处理器 | 完成 | generate、read、modify、convert、analyze |
| Excel 处理器 | 完成 | generate、read、modify、convert、analyze |
| PDF 处理器 | 完成 | 生成和读取功能 |
| PPT 处理器 | 完成 | generate、read、modify、convert、analyze |
| Markdown 处理器 | 完成 | generate、read、modify、convert、analyze |

### 下一步开发重点

1. **优先级高：Tauri 命令实现**
   - 实现 `start_agent`、`stop_agent` 命令的实际逻辑
   - 连接 Agent 执行器与数据库

2. **优先级高：LLM 适配器完善**
   - 实现 Claude 适配器
   - 实现 Gemini 适配器

3. **优先级中：Skill 系统完善**
   - 实现内置 Skills（word_skill、excel_skill 等）
   - 与 Sidecar 集成

4. **优先级中：前后端联调**
   - 测试事件流
   - 验证数据持久化

5. **优先级低：共享类型自动化**
   - 引入 ts-rs 或类似工具
   - 确保前后端类型同步

## 常用命令

```bash
# 启动 Vite 开发服务器（端口 1420）
npm run dev

# TypeScript 检查 + Vite 构建
npm run build

# 启动 Tauri 桌面应用开发模式
npm run tauri:dev

# Tauri 生产构建
npm run tauri:build
```

## 目录结构

```
src/                      # React 前端（约 3,689 行，完成度 90%）
├── components/
│   ├── layout/           # TopBar, MainLayout, MainArea, Sidebar, InputArea
│   ├── workflow/         # WorkflowTimeline + 7 种节点组件
│   ├── sidebar/          # FileTreeSection, AgentInfoSection, TodoSection, TokenSection
│   ├── preview/          # PreviewOverlay 预览面板
│   ├── settings/         # SettingsDialog + 5 个标签页
│   ├── session/          # HistoryPanel
│   └── common/           # Button, Icon
├── stores/               # 6 个 Zustand Store
├── types/                # 类型定义
├── utils/                # fileIcons, format
├── services/             # event.ts（事件封装）、tauri.ts（命令封装）
├── hooks/                # useAgent Hook
└── styles/globals.css    # Tailwind + 自定义设计令牌

src-tauri/                # Tauri Rust 后端（约 3,688 行，完成度 65%）
├── src/
│   ├── commands/         # Tauri 命令（部分实现）
│   ├── services/         # 服务层（llm、agent、skill）
│   ├── db/               # SQLite 数据库层（完整实现）
│   ├── config/           # 配置管理（完整实现）
│   ├── models/           # 数据模型定义
│   ├── events/           # 事件系统（完整实现）
│   └── utils/            # 工具函数
└── resources/            # 资源文件

sidecar/                  # Python Sidecar（约 1,217 行，完成度 85%）
├── main.py               # 入口，stdin/stdout JSON 协议通信
└── handlers/             # 文档处理器
    ├── word_handler.py   # Word 文档处理
    ├── excel_handler.py  # Excel 文档处理
    ├── pdf_handler.py    # PDF 文档处理
    ├── ppt_handler.py    # PPT 文档处理
    └── markdown_handler.py # Markdown 文档处理

shared/types.ts           # 前后端共享类型（NodeType, ExecutionStatus）
docs/                     # 设计文档（约 9,594 行）
├── PRD_DocAgent.md       # 产品需求文档
├── tech_architecture.md  # 技术架构文档
├── component_design.md   # 前端组件设计文档
├── database_design.md    # 数据库设计文档
├── tauri_commands.md     # Tauri 命令接口文档
├── skill_development.md  # Skill 系统开发规范
└── task_breakdown.md     # 开发任务分解文档
```

## 核心架构

### 通信方式
- `invoke()` — 请求-响应式调用（同步）
- `emit()/listen()` — 事件推送（流式输出、进度更新）
- **事件命名**：`namespace:action` 格式（如 `agent:thinking`, `session:updated`）

### Agent 执行流程
前端已预留完整的流式事件处理协议，后端 Agent 执行器核心逻辑已实现：
1. `agent:thinking` — LLM 思考链增量
2. `agent:content` — LLM 回复增量
3. `agent:tool_call` — Tool 调用开始
4. `agent:tool_result` — Tool 执行结果
5. `agent:confirm` — 需要用户确认
6. `agent:todo_update` — Todo 列表更新
7. `agent:done` — 执行完成
8. `agent:error` / `agent:stopped` — 错误/中断

### 状态管理
6 个 Zustand Store 职责分离：Workflow（节点）、Session（会话）、Workspace（工作区）、Settings（设置+LLM+Skill+模板）、FileTree（文件树）、Token（统计）。

### Python Sidecar
文档处理通过独立 Python 进程执行，与 Rust 后端通过 stdin/stdout JSON 协议通信。依赖：python-docx、openpyxl、python-pptx、reportlab、pdfkit。

### 数据库设计
SQLite 数据库包含 5 张表：
- `workspaces` — 工作区配置
- `sessions` — 会话记录
- `messages` — 消息历史
- `documents` — 文档元数据
- `skills` — Skill 注册表

## 开发注意事项

- **命名规范**：Tauri 命令用 `snake_case`，前端封装用 `camelCase`，事件名用 `namespace:action`
- **状态管理**：避免直接修改 store 中的数组/对象，使用不可变更新
- **组件优化**：工作流节点使用 React.memo，长列表使用虚拟滚动，搜索输入使用防抖
- **提交规范**：遵循 Conventional Commits 格式（feat/fix/docs/refactor/chore 等），使用中文描述
- **错误处理**：Rust 后端使用统一的 `AppError` 类型，前端通过事件接收错误信息
