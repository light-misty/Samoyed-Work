# 版本快照（回退消息）功能实施计划

**日期**: 2026-08-07
**设计文档**: `docs/plans/2026-08-07-version-snapshot-rollback-design.md`
**验证命令**: `cargo test` / `cargo clippy` / `npm run build`

## 架构

- 双引擎快照：git 仓库用 `git stash create`（只读），非 git 仓库用文件复制备份到 `app_data_dir/snapshots/{id}/`
- 每次 `start_agent`（用户发消息）前创建快照，关联该 user 消息；发射 `agent:snapshot_created` 事件，前端显示快照节点
- 回退 = staged 隐藏（消息不物理删除）+ 恢复文件到目标消息快照 + 回填输入框；支持 redo；新消息发送时物理删除隐藏消息
- 回退/redo 命令拒绝 Agent 运行中执行

---

### Task 1: DB 层（新表 + repos + message_repo 扩展）

**文件**:
- Modify: `src-tauri/src/db/init.rs`（建表 + 索引）
- Create: `src-tauri/src/db/snapshot_repo.rs`
- Create: `src-tauri/src/db/revert_repo.rs`
- Modify: `src-tauri/src/db/message_repo.rs`（新增按时间点函数）
- Modify: `src-tauri/src/db/mod.rs`

**Step 1 (RED)**: 先写 `snapshot_repo.rs` / `revert_repo.rs` / `message_repo.rs` 的 `#[cfg(test)]` 测试（内存 SQLite + `initialize_database`）：
- snapshot_repo: create/get_by_message_id/get_by_id/list_by_session/delete_by_session
- revert_repo: set/get/clear
- message_repo: `delete_messages_from`（删除 branch 内 created_at >= 目标的全部消息）、`list_messages_from`、`count_messages_from`

**Step 2**: 运行 `cargo test` 确认失败（函数不存在）。

**Step 3 (GREEN)**: 实现表结构与 repo 函数。

**Step 4**: `cargo test` 通过。

---

### Task 2: 快照服务 `services/snapshot.rs`

**文件**:
- Create: `src-tauri/src/services/snapshot.rs`
- Modify: `src-tauri/src/services/mod.rs`

**功能**:
- `enum SnapshotKind { Git, Files }`（serde rename 为 "git"/"files"）
- `create_snapshot(workspace_path, backup_base_dir) -> Result<(SnapshotKind, String)>`：
  - 检测 `git -C <ws> rev-parse --is-inside-work-tree` 输出 "true"
  - git: `git -C <ws> stash create`（空输出 → `git rev-parse HEAD`；仍失败 → 降级 files）
  - files: 遍历工作区复制到 `backup_base_dir/{uuid}/`，跳过黑名单目录 `.git node_modules target dist out build .venv venv __pycache__ .idea .vscode`
- `restore_snapshot(kind, snapshot_ref, workspace_path, paths) -> Result<usize>`：
  - git: `git -C <ws> restore --source=<sha> --worktree -- <paths...>`（paths 空 → 返回 0）
  - files: 对每个 path，备份中存在 → 复制回工作区；不存在 → 删除工作区文件
- `delete_backup_dir(snapshot_ref)`（files 方式清理，忽略错误）
- `collect_tool_paths(messages: &[Message]) -> Vec<String>`：按工具名提取 `path` / `source_path` / `target_path`，去重
  - write/edit/remove/mkdir/remove_dir → `path`
  - rename/copy → `source_path` + `target_path`

**Step 1 (RED)**: 测试：
- `test_collect_tool_paths`：各工具参数提取 + 去重
- `test_files_snapshot_roundtrip`：tempdir 建工作区文件 → files 快照 → 修改文件 + 新建文件 → restore → 验证恢复
- `test_git_snapshot_roundtrip`（git 可用时）：tempdir git init + 提交 → 修改文件 + 新建文件 → stash create 快照 → restore → 验证恢复（git 不可用则跳过）
- 测试用临时目录，不触碰真实工作区

**Step 2**: `cargo test` 失败。
**Step 3 (GREEN)**: 实现。
**Step 4**: `cargo test` 通过。

---

### Task 3: 模型 + 事件

**文件**:
- Create: `src-tauri/src/models/snapshot.rs`（`SnapshotRecord`、`RevertRecord`、`RollbackResult`、`SnapshotInfo`，camelCase serde）
- Modify: `src-tauri/src/models/mod.rs`
- Modify: `src-tauri/src/events/types.rs`（`AGENT_SNAPSHOT_CREATED` + `SnapshotCreatedPayload`）
- Modify: `src-tauri/src/events/emitter.rs`（`emit_snapshot_created`）

**Step 1 (RED)**: 模型序列化测试（camelCase 字段名）。
**Step 2-4**: 实现 + 通过。

---

### Task 4: 命令（session.rs）

**文件**:
- Modify: `src-tauri/src/commands/session.rs`
- Modify: `src-tauri/src/lib.rs`（注册命令）

**新增命令**:
- `rollback_session_messages(session_id, message_id) -> RollbackResult`：
  1. Agent 运行中拒绝
  2. 校验目标消息存在、role=user、属于当前活跃分支；获取其 created_at
  3. 查消息快照（snapshot_repo::get_by_message_id）：
     - 有 → 提取边界及之后消息的工具路径 → restore → restored_file_count
     - 无 → restored_file_count=0，`code_reverted=false`
  4. redo 基线：已有 revert 记录则保留原 redo_snapshot_id；否则创建当前文件快照（message_id=NULL）存入 DB
  5. hidden_count = 边界及之后消息数（count_messages_from）
  6. revert_repo::set（revert_message_id, redo_snapshot_id）
  7. 返回 RollbackResult { revertMessageId, hiddenCount, restoredFileCount, codeReverted, snapshotKind }
- `redo_session_messages(session_id) -> RedoResult { hiddenCount }`：
  1. Agent 运行中拒绝；无 revert 记录 → 返回 hiddenCount=0
  2. 提取隐藏消息范围工具路径 → restore redo 基线快照
  3. revert_repo::clear + 删除 redo 快照记录（message_id IS NULL 的）
  4. 返回恢复的消息数（边界及之后消息数）

**修改**:
- `get_session`：返回 `revert: Option<RevertInfo>`（revertMessageId/hiddenCount/snapshotKind，仅当边界消息属于当前活跃分支）、`snapshots: Vec<SnapshotInfo>`（按 message_id 关联，供前端恢复快照节点）
- `delete_session` / `clear_workspace_sessions` / `clear_all_sessions`：级联删除 snapshot_repo/revert_repo 记录

**测试**: 命令层不易单测（依赖 AppState），repo 层测试已覆盖；命令逻辑通过 `cargo build` + 手动验证。

---

### Task 5: start_agent 快照创建 + cleanup

**文件**:
- Modify: `src-tauri/src/commands/agent.rs`

**逻辑**:
- start_agent 的 tokio::spawn 内、run_agent 调用前：
  1. 若存在 staged revert → cleanup：`delete_messages_from` 物理删除隐藏消息、revert_repo::clear、删除 redo 快照记录与备份目录
  2. 创建快照（`create_snapshot`，workspace_path 非空且非 "." 时），失败仅 warn（不影响发送）
  3. 快照以 message_id=NULL 暂存
- run_agent 返回后（Ok/Err 均执行）：查询该分支最新 user 消息 ID，回填快照 message_id；发射 `agent:snapshot_created` 事件（payload 含 snapshotId/kind/sessionId）
- AppState 增加 snapshot 备份根目录（`app_data_dir/snapshots`），构造时传入（lib.rs 中初始化）

---

### Task 6: 前端类型 + tauri.ts

**文件**:
- Modify: `src/types/session.ts`（`SessionDetail.revert`、`SessionDetail.snapshots`、`RollbackResult`、`SnapshotInfo`、`RevertInfo`）
- Modify: `src/services/tauri.ts`（`rollbackSessionMessages` / `redoSessionMessages` 封装）

---

### Task 7: UserNode 回退按钮 + 确认弹窗

**文件**:
- Modify: `src/components/workflow/UserNode.tsx`

**逻辑**:
- `wf-branch-button` 左侧插入回退按钮（icon `history`，class `wf-rollback-button`，`isAgentRunning` 时禁用）
- 点击 → 内联确认弹窗（仿 `wf-del-dialog`，含代码回退说明）
- 确认 → `handleRollback`：
  1. 解析 messageId（节点 data.messageId；实时节点按位置从后端匹配，复用删除消息的定位逻辑）
  2. `tauriCmd.rollbackSessionMessages(sessionId, messageId)`
  3. `Promise.all([listBranchGroups, getSession])` → `loadFromMessages` 刷新工作流
  4. `clearSessionCache` + `loadContextUsage`
  5. `useSettingsStore.getState().setPendingInsertTemplate(data.content)` 回填输入框
- 无快照（codeReverted=false）时在提示中说明"代码未回退"

---

### Task 8: 快照节点 + 回退 banner

**文件**:
- Modify: `src/types/workflow.ts`（`SnapshotNodeData` + `NodeDataMap` + `WorkflowNodeType` 加 `"snapshot"`）
- Create: `src/components/workflow/SnapshotNode.tsx`（tool 样式轻量节点："文件快照已创建"）
- Modify: `src/components/workflow/WorkflowNode.tsx`（case "snapshot"）
- Modify: `src/components/workflow/WorkflowTimeline.tsx`（顶部 banner：已回退 N 条消息 + 撤销回退按钮；banner 不显示时正常渲染）
- Modify: `src/stores/useWorkflowStore.ts`：
  - state 增加 `revertInfo`（RevertInfo | null）+ `setRevertInfo`
  - `loadFromMessages` 从 messages + snapshots 恢复快照节点（user 节点后插入 snapshot 节点）；从 detail.revert 设置 revertInfo
  - `clearNodes` 重置 revertInfo
- Modify: `src/hooks/useAgent.ts`（监听 `onAgentSnapshotCreated` → `addNode("snapshot", ...)`）
- Modify: `src/services/event.ts`（`onAgentSnapshotCreated` 封装）

**redo 流程**: banner 撤销按钮 → `tauriCmd.redoSessionMessages` → 刷新工作流（同回退流程 3-4 步）

---

### Task 9: i18n + 样式

**文件**:
- Modify: `src/i18n/locales/zh-CN.json`、`src/i18n/locales/en-US.json`（workflow 段新增：rollback / rollbackConfirm / rollbackSuccess / rollbackNoCode / snapshotCreated / revertedBanner / redoRollback 等）
- Modify: `src/styles/globals.css`（`.wf-rollback-button`、`.wf-snapshot-node`、`.wf-revert-banner` 等）

---

### Task 10: 全量验证

1. `cargo test`（全部通过，含新增测试）
2. `cargo clippy`（无新增 warning）
3. `cargo fmt --check`
4. `npm run build`（tsc + vite 通过）
5. 手动验证清单（写在后）：
   - 发消息 → 快照节点出现
   - 回退按钮 → 确认弹窗 → 工作流回退 + 文字回填 + banner 出现
   - 撤销回退（redo）→ 消息恢复 + 文件恢复
   - 回退后发新消息 → 隐藏消息物理删除、banner 消失
   - 跨消息回退（倒数第二条）→ 两条消息及之间全部回退
   - Agent 运行中按钮禁用
   - 分支：回退只影响当前分支，切分支正常
   - 删除消息功能不受影响
