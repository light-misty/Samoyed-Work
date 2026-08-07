# 版本快照（回退消息）功能设计

**日期**: 2026-08-07
**状态**: 已确认，待实施

## 目标

在会话页面用户消息节点下方（"修改并创建分支"按钮左侧）新增"回退"按钮。用户确认回退后：
1. 工作流回退到该节点发送之前（该节点及其后的所有消息从视图中移除）
2. 该节点文字回填输入框
3. 被回退范围内的上下文删除（消息隐藏即从上下文消失）
4. 被回退范围内智能体修改的代码回退（通过快照恢复）
5. 支持跨消息节点回退（选中倒数第 2 条用户消息时，倒数第 1、2 条及之间全部回退）
6. 与"修改并创建分支"、"删除消息"功能无冲突

## 核心机制：双引擎快照（参考 OpenCode revert 实现）

### 快照类型

| 类型 | 适用场景 | 创建方式 | 恢复方式 |
|------|---------|---------|---------|
| `git` | 工作区是 git 仓库 | `git stash create`（只读，不污染 index/分支/refs），返回 commit SHA；无修改时回退到 `git rev-parse HEAD` | `git restore --source=<sha> --worktree -- <paths>`（不修改暂存区） |
| `files` | 非 git 仓库 | 文件复制备份到 `app_data_dir/snapshots/{id}/`，跳过 `.git/node_modules/target/dist/out/build/.venv/venv/__pycache__/.idea/.vscode` 目录 | 备份中存在则复制回工作区，不存在则删除工作区文件 |

### 快照创建时机

每次用户发送消息（`start_agent` 执行前、agent 工具执行前）创建快照，快照状态 = 该消息发送前的文件状态。快照关联该 user 消息 ID。快照创建成功后发射 `agent:snapshot_created` 事件，前端在用户节点后添加"文件快照已创建"节点（满足"创建备份的工作流节点"需求）。

### 快照范围限制

快照功能上线前发送的旧消息无快照 → 回退时仅回退对话不回退代码，并在结果提示中说明"代码未回退（该消息无可用的文件快照）"。

## 回退流程（staged 模式，支持 redo）

```
用户点击回退按钮 → 确认弹窗 → 确认
  ↓
1. 创建 redo 基线快照（当前文件状态，用于撤销回退）
2. 记录 session_reverts 状态（revert_message_id = 选中的 user 消息）
3. 文件恢复：提取被回退范围内工具调用涉及的路径，
   用目标消息的快照恢复到这些路径
4. 前端刷新工作流：只显示回退边界之前的消息
5. 选中的 user 消息文字回填输入框（pendingInsertTemplate 机制）
  ↓
工作流顶部显示横幅："已回退 N 条消息 [撤销回退]"
  ↓
redo（撤销回退）：恢复 redo 基线快照文件 + 清除 revert 状态 + 消息恢复显示
  ↓
用户发送新消息：真正物理删除被隐藏的消息（cleanup），清除 revert 状态
```

关键点：
- 回退后消息**隐藏**而非物理删除（redo 需要）；物理删除延迟到新消息发送时
- 回退时先备份当前文件状态（redo 基线），再恢复到目标快照——redo 时恢复的是回退前的精确状态
- 文件恢复范围：仅被回退消息范围内工具调用（write/remove/rename/copy/edit 等）涉及的路径，用户手动修改的文件不受影响（与 OpenCode 一致）
- 上下文删除 = 消息隐藏，隐藏的消息不会进入 LLM 上下文（`get_session` 只返回边界前的消息）

## 数据层

### 新表 `session_snapshots`

```sql
CREATE TABLE IF NOT EXISTS session_snapshots (
    id             TEXT NOT NULL PRIMARY KEY,
    session_id     TEXT NOT NULL,
    message_id     TEXT,            -- 关联的 user 消息；NULL 表示 redo 基线快照
    kind           TEXT NOT NULL,   -- 'git' | 'files'
    snapshot_ref   TEXT NOT NULL,   -- git SHA 或备份目录路径
    workspace_path TEXT NOT NULL,   -- 快照时的工作区路径
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

### 新表 `session_reverts`

```sql
CREATE TABLE IF NOT EXISTS session_reverts (
    session_id       TEXT NOT NULL PRIMARY KEY,
    revert_message_id TEXT NOT NULL,   -- 回退边界：该消息及之后隐藏
    redo_snapshot_id TEXT NOT NULL,    -- redo 基线快照 ID
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
```

### 级联清理

删除会话（`delete_session`/`clear_*`）时同步删除两个表的记录。

## 后端命令

### 新命令

- `rollback_session_messages(session_id, message_id)` -> `RollbackResult`
  - Agent 运行中拒绝
  - 校验 message_id 属于当前活跃分支且是 user 消息
  - 若已有 staged revert（重复回退）：redo 基线保留首次的（OpenCode 行为），直接更新边界
  - 创建 redo 基线快照 → 恢复文件（目标消息快照 + 路径列表）→ 写 session_reverts
  - 返回：revert_message_id、隐藏消息数、恢复文件数、快照类型（无快照时为 None → 前端提示仅回退对话）
- `redo_session_messages(session_id)`
  - Agent 运行中拒绝；无 revert 状态时 no-op
  - 恢复文件到 redo 基线快照 → 删除 session_reverts → 返回恢复的隐藏消息数

### 修改现有命令

- `get_session`：若存在 staged revert 且边界消息属于当前活跃分支，`messages` 只返回边界之前的消息，并返回 `revert` 信息（revertMessageId、隐藏消息数、redoSnapshotKind）
- `start_agent`：agent 执行前创建快照并关联 user 消息（user 消息持久化后回填 message_id），发射 `agent:snapshot_created` 事件；若存在 staged revert，先执行 cleanup（物理删除隐藏消息、删除 revert 状态与 redo 基线快照记录）

## 工具路径提取

从被回退范围内的消息 tool_calls 中提取工作区文件路径：

| 工具 | 参数 |
|------|------|
| write / edit / remove / create_directory / delete_directory | `path` |
| rename / copy | `source_path` + `target_path` |
| 其他（bash 等） | 不提取（有副作用，无法精确恢复，按 OpenCode 设计接受局限） |

路径收集后先去重，再按"快照后新建 → 删除"或"快照中存在 → 覆盖恢复"处理。

## 前端改动

### 组件

- `UserNode.tsx`：`wf-branch-button` 左侧新增回退按钮（icon `history`），点击打开确认弹窗（仿现有 `wf-del-dialog` 样式）；确认后调用 `rollbackSessionMessages` → 刷新工作流（`loadFromMessages`）→ `pendingInsertTemplate` 回填该消息文字 → `clearSessionCache` + `loadContextUsage`
- 快照节点：`snapshot_created` 事件监听（复用 `useAgent` 事件流或新增监听），在 user 节点后添加轻量节点（tool 样式，显示"文件快照已创建"）；`loadFromMessages` 从 user 消息 metadata 恢复快照节点（user 消息持久化时写入 `snapshot: {kind, createdAt}`）
- 回退状态 banner：会话工作流顶部显示"已回退 N 条消息 [撤销回退]"，点击调用 `redoSessionMessages` 后刷新工作流

### 状态

- `types/session.ts`：`SessionDetail` 增加 `revert?: { revertMessageId, hiddenCount, redoSnapshotKind }`；`Message` 增加 `metadata`（现有）与快照相关字段
- `services/tauri.ts`：新增 `rollbackSessionMessages` / `redoSessionMessages` 封装
- `useWorkflowStore`：`loadFromMessages` 支持 metadata 中快照节点恢复；新增 `revertInfo` 状态

### 样式与文案

- `globals.css`：`.wf-rollback-button`、快照节点、banner 样式
- i18n：zh-CN / en-US 新增回退按钮、确认弹窗、回退成功提示、banner 文案

## 与现有功能的关系

| 功能 | 冲突处理 |
|------|---------|
| 删除消息 | 独立链路。回退使用 staged 隐藏，物理删除延迟到新消息发送；删除消息功能不受影响 |
| 修改并创建分支 | 回退仅作用于当前活跃分支（branch_id 过滤）；不删除 message_branches 记录；分支切换时若 revert 边界不属于当前分支则忽略 revert 状态；回退按钮与分支按钮并排但逻辑独立 |
| 上下文 | 隐藏消息不进上下文；新消息发送时清理隐藏消息，上下文精确对齐 |

## 局限（与用户确认）

1. 快照功能上线前的旧消息无快照 → 仅回退对话，提示代码未回退
2. bash 等有副作用的工具无法精确回退（路径提取之外的操作不恢复）
3. 文件备份方式（非 git 仓库）复制开销随工作区大小增长（跳过常见大目录）
4. 回退按钮仅对用户消息节点显示

## 测试计划

- `snapshot.rs` 单测：git 快照创建/恢复（临时 git 仓库）、文件备份快照创建/恢复、无修改快照
- `message_repo` 单测：按时间点删除消息、按时间点查询消息（内存 SQLite）
- `tool_paths.rs` 单测：路径提取（各工具参数）
- 命令层：rollback/redo 幂等性、Agent 运行中拒绝（如可测）
- 前端：`npm run build` 通过（无前端测试框架）
