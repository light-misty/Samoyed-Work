# Code Handler 代码预览实时流式显示 & 自动执行设计文档

**日期**: 2026-06-14
**状态**: 待评审
**作者**: Agent 全栈开发工程师

---

## 1. 需求概述

### 1.1 当前行为

当用户提问后，智能体调用 `code_interpreter_handler` 时，代码预览卡片的显示时机和交互流程如下：

1. LLM 流式输出 `tool_call` 参数（含 `code` 字段），后端在检测到工具名称后立即发射 `agent:tool_call` 事件
2. 前端收到 `agent:tool_call` 后创建 `type: "tool"` 的 WorkflowNode，状态为 `running`，仅显示工具名称和简要描述（如"执行代码 生成Word文档"）
3. 由于 `code_interpreter_handler` 在 `HIGH_RISK_HANDLERS` 列表中，后端发射 `agent:confirm` 事件，前端弹出 `ConfirmNode`，其中包含代码预览区域（截断显示，可展开）
4. 用户点击确认后，后端执行代码，执行完成后发射 `agent:tool_result` 事件
5. 前端收到 `agent:tool_result` 后更新 ToolNode 状态为 `completed` 或 `failed`

**核心问题**：
- 代码预览仅在 ConfirmNode 中出现，且仅在确认阶段可见。用户无法在 LLM 编写代码的过程中实时看到代码内容，体验割裂
- 用户必须手动点击确认才能执行代码，增加了不必要的交互步骤，降低了效率

### 1.2 目标行为

1. **即时显示**：智能体开始编写代码后（即 LLM 流式输出 `code_interpreter_handler` 的 `tool_call` 参数时），即刻在 ToolNode 中显示代码预览卡片
2. **初始展开**：代码预览卡片一开始处于展开状态
3. **实时流式显示**：LLM 编写的代码在代码预览中实时流式显示，逐字/逐行呈现
4. **完成后收缩**：代码编写完成后（`agent:tool_result` 事件到达），代码预览卡片自动切换为收缩状态
5. **明显高度差**：收缩状态的代码预览框高度明显小于展开状态，形成清晰的视觉对比
6. **自动执行**：代码编写完成后自动执行，不再需要用户点击确认
7. **删除确认功能**：移除 `code_interpreter_handler` 的用户确认流程，包括 ConfirmNode 弹出、确认/拒绝按钮、确认超时等

---

## 2. 现有架构分析

### 2.1 后端确认机制

当前确认机制的完整代码链路：

#### 2.1.1 高风险 Handler 列表

```rust
// src-tauri/src/services/agent/executor.rs:25
const HIGH_RISK_HANDLERS: &[&str] = &["delete_file", "code_interpreter_handler"];
```

`code_interpreter_handler` 被列为高风险 Handler，在 `EditOnly` 确认级别下需要用户确认。

#### 2.1.2 确认级别配置

```rust
// src-tauri/src/config/app_settings.rs
pub enum ConfirmationLevel {
    Always,      // 所有操作都需要确认
    EditOnly,    // 仅高风险操作需要确认（默认）
    Never,       // 任何操作都不需要确认
}
```

#### 2.1.3 确认判断逻辑

```rust
// executor.rs:185-198
fn needs_confirmation(&self, name: &str, _params: &serde_json::Value) -> bool {
    match self.confirmation_level {
        ConfirmationLevel::Never => false,
        ConfirmationLevel::EditOnly => {
            if HIGH_RISK_HANDLERS.contains(&name) {
                return true;
            }
            false
        }
        ConfirmationLevel::Always => true,
    }
}
```

#### 2.1.4 确认请求流程

```rust
// executor.rs:1068-1100
if self.needs_confirmation(&tool_call.name, &params) {
    // 发射 tool_call 事件（带"等待确认"标记）
    self.emitter.emit_tool_call(ToolCallPayload {
        tool_name: format!("{} (等待确认)", tool_call.name),
        ...
    });

    // 发射 confirm 事件，等待用户响应
    let approved = self.request_confirmation(...).await?;

    if !approved {
        // 用户拒绝：发射 tool_result (success: false)
        // 跳过执行
        continue;
    }
} else {
    // 普通工具：直接发射 tool_call 事件
    self.emitter.emit_tool_call(ToolCallPayload {
        tool_name: tool_call.name.clone(),
        ...
    });
}
```

#### 2.1.5 确认超时机制

```rust
// executor.rs:309-338
// 等待用户确认，超时 300 秒
match tokio::time::timeout(Duration::from_secs(CONFIRM_TIMEOUT_SECS), rx).await {
    Ok(Ok(decision)) => { /* 用户确认/拒绝 */ }
    Ok(Err(_)) => { /* 通道关闭 */ }
    Err(_) => { /* 超时 */ }
}
```

### 2.2 前端确认机制

#### 2.2.1 useAgent hook 中的确认处理

```typescript
// src/hooks/useAgent.ts
// 状态
const [pendingConfirmation, setPendingConfirmation] = useState<ConfirmPayload | null>(null);

// 事件监听
onAgentConfirm((payload) => {
    if (payload.sessionId !== sessionIdRef.current) return;
    setPendingConfirmation(payload);
}),

// 确认操作回调
const confirmOperation = useCallback(async (operationId, approved, feedback) => {
    await tauriCmd.confirmOperation(sessionId, operationId, approved, feedback);
    setPendingConfirmation(null);
}, [sessionId]);
```

#### 2.2.2 App.tsx 中的 ConfirmNode 创建

```typescript
// src/App.tsx:504-542
useEffect(() => {
    if (pendingConfirmation) {
        // 从 details 中提取代码
        const details = pendingConfirmation.details as Record<string, unknown>;
        const code = details?.code as string | undefined;
        // 分离描述和代码预览
        let displayDescription = pendingConfirmation.description;
        if (code) {
            const newlineIdx = displayDescription.indexOf('\n');
            if (newlineIdx !== -1) {
                displayDescription = displayDescription.substring(0, newlineIdx);
            }
        }
        // 创建 ConfirmNode
        const confirmData = {
            title: pendingConfirmation.operationType,
            description: displayDescription,
            confirmLabel: t('confirmNode.confirmExecute'),
            cancelLabel: t('confirmNode.cancelOperation'),
            confirmed: null,
            ...(code ? { code } : {}),
        };
        const nodeId = addNode("confirm", confirmData, "running");
        confirmNodeIdRef.current = nodeId;

        // 注册确认回调
        setConfirmHandler(async (approved: boolean) => {
            if (confirmNodeIdRef.current) {
                updateNode(confirmNodeIdRef.current, {
                    data: { ...confirmData, confirmed: approved },
                    status: approved ? "completed" : "cancelled",
                });
                confirmNodeIdRef.current = null;
            }
            await confirmOperation(pendingConfirmation.operationId, approved);
            setConfirmHandler(null);
        });
    }
}, [pendingConfirmation, addNode, updateNode, confirmOperation, setConfirmHandler]);
```

#### 2.2.3 ConfirmNode 组件

```tsx
// src/components/workflow/ConfirmNode.tsx
// 包含：标题、描述、代码预览区域（截断显示，可展开）、确认/拒绝按钮
// 代码预览区域仅在有 code 字段时显示
```

#### 2.2.4 设置页面中的确认级别选项

```tsx
// src/components/settings/GeneralTab.tsx:79-93
// 确认级别下拉选择：始终确认 / 仅编辑操作 / 从不确认
<select value={settings.general.confirmationLevel}>
    <option value="always">始终确认</option>
    <option value="editOnly">仅编辑操作</option>
    <option value="never">从不确认</option>
</select>
```

### 2.3 后端事件流

当前 `code_interpreter_handler` 调用时的事件时序：

```
LLM 流式输出 tool_call 参数
  |
  +-- 检测到工具名称 -> emit agent:tool_call (参数可能不完整)
  |   ToolCallPayload { callId, toolName, arguments: { code: "部分代码..." } }
  |
  +-- 流式结束 -> emit agent:tool_call (参数完整，前端通过 callId 去重更新)
  |   ToolCallPayload { callId, toolName: "code_interpreter_handler (等待确认)", arguments: { code: "完整代码" } }
  |
  +-- 需要确认 -> emit agent:confirm
  |   ConfirmPayload { operationId, operationType: "code_interpreter_handler", details: { code: "完整代码" } }
  |
  +-- [用户点击确认] -> confirm_operation(approved: true)
  |
  +-- 执行代码...
  |
  +-- 执行完成 -> emit agent:tool_result
      ToolResultPayload { callId, success, result, error }
```

### 2.4 前端数据流

当前 `agent:tool_call` 事件的处理链路：

1. `useAgent.ts` 中 `onAgentToolCall` 监听器接收 `ToolCallPayload`
2. 设置 `currentToolCall` 状态
3. `App.tsx` 中 `useEffect([currentToolCall])` 处理：
   - 通过 `callId` 查找已有 ToolNode -> 若存在则更新参数
   - 若不存在 -> 关闭 thinking/streaming 节点，创建新 ToolNode（status: "running"）
4. `ToolNode.tsx` 渲染：仅显示工具名称 + 简要描述 + 执行状态

### 2.5 当前 ToolNodeData 结构

```typescript
// src/types/workflow.ts
export interface ToolNodeData {
  toolName: string;
  briefDescription: string;
  input: Record<string, unknown>;
  callId?: string;
  success?: boolean;
  error?: string;
}
```

`input` 字段包含完整的工具参数（含 `code` 字段），但当前 ToolNode 组件并未利用此字段渲染代码预览。

### 2.6 当前 ToolNode 渲染逻辑

ToolNode 当前仅渲染一行简要信息（工具名称 + 简要描述 + 执行状态），无代码预览区域。

### 2.7 ConfirmNode 中的代码预览

ConfirmNode 已有代码预览的 UI 实现，但它是静态的、截断式的，不支持流式显示。随着确认功能的移除，此代码预览将不再需要。

---

## 3. 设计方案

### 3.1 总体思路

核心改动分为四层：

1. **后端层 - 代码流式事件**：新增 `agent:code_streaming` 事件，在 LLM 流式输出 `code_interpreter_handler` 参数的 `code` 字段时，逐块发射代码增量
2. **后端层 - 移除确认**：将 `code_interpreter_handler` 从 `HIGH_RISK_HANDLERS` 中移除，代码编写完成后自动执行
3. **数据层**：扩展 `ToolNodeData` 类型，增加代码流式状态字段；扩展 workflow store 处理新事件
4. **前端层**：改造 ToolNode 组件，增加代码预览区域，支持展开/收缩/流式显示；移除 ConfirmNode 对 `code_interpreter_handler` 的处理

### 3.2 后端改动

#### 3.2.1 新增事件类型 `agent:code_streaming`

在 `src-tauri/src/events/types.rs` 中新增：

```rust
/// 代码流式增量事件（仅 code_interpreter_handler 触发）
/// 当 LLM 流式输出 code_interpreter_handler 的 code 参数时，
/// 逐块发射代码增量，前端可实时显示代码编写过程
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CodeStreamingPayload {
    pub session_id: String,
    /// 关联的 tool_call ID，用于前端匹配对应的 ToolNode
    pub call_id: String,
    /// 代码增量内容（delta，非全量）
    pub code_delta: String,
    /// 是否为流式输出的最后一个事件
    pub is_final: bool,
}
```

在 `src-tauri/src/events/types.rs` 中新增事件名常量：

```rust
pub const AGENT_CODE_STREAMING: &str = "agent:code_streaming";
```

在 `src-tauri/src/events/emitter.rs` 中新增发射方法：

```rust
/// 发射代码流式增量事件（非关键，高频流式）
pub fn emit_code_streaming(&self, payload: types::CodeStreamingPayload) -> Result<(), CommandError> {
    self.emit_event(types::AGENT_CODE_STREAMING, payload, false, true)
}
```

#### 3.2.2 移除 code_interpreter_handler 的确认机制

**改动 1：从 HIGH_RISK_HANDLERS 中移除**

```rust
// src-tauri/src/services/agent/executor.rs:25
// 修改前：
const HIGH_RISK_HANDLERS: &[&str] = &["delete_file", "code_interpreter_handler"];
// 修改后：
const HIGH_RISK_HANDLERS: &[&str] = &["delete_file"];
```

**改动 2：移除 request_confirmation 中 code_interpreter_handler 的特殊处理**

```rust
// executor.rs request_confirmation 方法中
// 修改前：
let risk_level = match self.confirmation_level {
    ConfirmationLevel::Always => {
        if tool_name == "delete_file" {
            "critical"
        } else if tool_name == "code_interpreter_handler" {
            "high"  // 代码执行始终为高风险
        } else {
            "normal"
        }
    }
    _ => {
        if tool_name == "delete_file" {
            "critical"
        } else {
            "high"
        }
    }
};

let description = match tool_name {
    // ...
    "code_interpreter_handler" => {
        let desc = arguments["description"].as_str().unwrap_or("执行代码");
        let code_preview: String = arguments["code"].as_str()
            .map(|c| { /* 截断逻辑 */ })
            .unwrap_or_default();
        format!("执行代码: {}\n{}", desc, code_preview)
    }
    // ...
};

// 修改后：移除 code_interpreter_handler 分支
let risk_level = match self.confirmation_level {
    ConfirmationLevel::Always => {
        if tool_name == "delete_file" {
            "critical"
        } else {
            "normal"
        }
    }
    _ => {
        if tool_name == "delete_file" {
            "critical"
        } else {
            "high"
        }
    }
};

let description = match tool_name {
    "delete_file" => format!("删除文件: {}", arguments["path"].as_str().unwrap_or("未知")),
    "docx_handler" | "xlsx_handler" | "pptx_handler" | "pdf_handler" => {
        let action = arguments["action"].as_str().unwrap_or("操作");
        let path = arguments["path"].as_str().unwrap_or("未知文件");
        format!("{} - {}: {}", tool_name, action, path)
    }
    _ => format!("执行操作: {}", tool_name),
};
```

**改动 3：移除 needs_confirmation 中对 code_interpreter_handler 的隐式影响**

由于 `code_interpreter_handler` 已从 `HIGH_RISK_HANDLERS` 中移除，`needs_confirmation` 方法在 `EditOnly` 级别下将不再对 `code_interpreter_handler` 返回 `true`。无需额外修改此方法。

**改动 4：移除 executor 主循环中"等待确认"标记**

```rust
// executor.rs:1068-1100
// 修改前：
if self.needs_confirmation(&tool_call.name, &params) {
    self.emitter.emit_tool_call(ToolCallPayload {
        tool_name: format!("{} (等待确认)", tool_call.name),
        ...
    });
    let approved = self.request_confirmation(...).await?;
    if !approved { continue; }
} else {
    self.emitter.emit_tool_call(ToolCallPayload {
        tool_name: tool_call.name.clone(),
        ...
    });
}

// 修改后（code_interpreter_handler 不再需要确认，走 else 分支）：
// 无需修改此段代码逻辑，因为 needs_confirmation 对 code_interpreter_handler 已返回 false
// 但需确认：当 confirmation_level 为 Always 时，code_interpreter_handler 仍需确认
// 根据需求"删除有关用户点击确认的功能"，应理解为仅针对 code_interpreter_handler 删除确认
// 因此在 Always 级别下，code_interpreter_handler 也应跳过确认
```

**进一步细化**：需求明确要求"代码生成后自动执行，不再由用户点击确认执行，删除有关用户点击确认的功能"。这意味着 `code_interpreter_handler` 在**任何确认级别**下都不需要确认。需要在 `needs_confirmation` 中显式排除：

```rust
fn needs_confirmation(&self, name: &str, _params: &serde_json::Value) -> bool {
    // code_interpreter_handler 始终不需要确认（代码自动执行）
    if name == "code_interpreter_handler" {
        return false;
    }
    match self.confirmation_level {
        ConfirmationLevel::Never => false,
        ConfirmationLevel::EditOnly => {
            if HIGH_RISK_HANDLERS.contains(&name) {
                return true;
            }
            false
        }
        ConfirmationLevel::Always => true,
    }
}
```

#### 3.2.3 修改 executor.rs 流式处理逻辑

在 `src-tauri/src/services/agent/executor.rs` 的流式响应收集循环中，当检测到 `code_interpreter_handler` 的 `tool_call` 参数正在流式输出时，解析 `arguments` 中的 `code` 字段增量，发射 `agent:code_streaming` 事件。

**核心逻辑**：

```
流式收集 tool_call 参数增量时：
  1. 检测到工具名称为 code_interpreter_handler
  2. 尝试从已收集的 arguments 字符串中提取 code 字段的增量
  3. 发射 agent:code_streaming 事件，携带 code_delta
  4. 流式结束时发射 is_final=true 的事件
```

**具体实现策略**：

由于 LLM 流式输出的 `arguments` 是 JSON 字符串的增量拼接，直接解析可能因 JSON 不完整而失败。采用以下策略：

- 维护一个 `prev_code_length` 变量，记录上次成功发射的 code 长度
- 每当 `arguments` 增量到达时，尝试解析完整 JSON 提取 `code` 字段
- 如果解析成功且 `code` 长度 > `prev_code_length`，发射增量 `code[prev_code_length..]`
- 如果解析失败（JSON 不完整），暂不发射，等待下次增量
- 流式结束时，强制发射 `is_final=true` 事件，确保前端收到结束信号

**修改位置**：`executor.rs` 中流式响应收集循环的 `delta_tool_calls` 处理分支内。

```rust
// 在流式收集 tool_calls 增量的循环中，新增代码流式发射逻辑
if let Some(delta_tool_calls) = choice.delta.tool_calls {
    for tc in delta_tool_calls {
        // ... 现有的参数合并逻辑 ...

        // 新增：当检测到 code_interpreter_handler 时，发射代码流式增量
        if collected.name == "code_interpreter_handler" {
            if let Ok(params) = serde_json::from_str::<serde_json::Value>(&collected.arguments) {
                if let Some(code) = params.get("code").and_then(|v| v.as_str()) {
                    let prev_len = code_streaming_state
                        .get(&tc_index)
                        .copied()
                        .unwrap_or(0);
                    if code.len() > prev_len {
                        let delta = &code[prev_len..];
                        self.emitter.emit_code_streaming(CodeStreamingPayload {
                            session_id: ctx.session_id.clone(),
                            call_id: if collected.id.is_empty() {
                                format!("streaming_{}", tc_index)
                            } else {
                                collected.id.clone()
                            },
                            code_delta: delta.to_string(),
                            is_final: false,
                        }).ok();
                        code_streaming_state.insert(tc_index, code.len());
                    }
                }
            }
        }
    }
}
```

需要在循环外声明 `code_streaming_state`：

```rust
let mut code_streaming_state: HashMap<u32, usize> = HashMap::new();
```

流式结束时，发射 `is_final=true`：

```rust
// 在流式循环结束后
// 为所有 code_interpreter_handler 的 tool_call 发射 is_final 事件
for (tc_index, collected) in collected_tool_calls.iter() {
    if collected.name == "code_interpreter_handler" {
        self.emitter.emit_code_streaming(CodeStreamingPayload {
            session_id: ctx.session_id.clone(),
            call_id: if collected.id.is_empty() {
                format!("streaming_{}", tc_index)
            } else {
                collected.id.clone()
            },
            code_delta: String::new(),
            is_final: true,
        }).ok();
    }
}
```

**注意**：`collected_tool_calls` 在流式收集阶段是 `HashMap<u32, LlmToolCall>`，后续才转为 `Vec`。`is_final` 事件应在 HashMap 阶段发射，在转为 Vec 之前。

### 3.3 前端事件层改动

#### 3.3.1 新增 Payload 类型

在 `src/services/event.ts` 中新增：

```typescript
/** 代码流式增量事件（仅 code_interpreter_handler 触发） */
export interface CodeStreamingPayload {
  sessionId: string;
  /** 关联的 tool_call ID */
  callId: string;
  /** 代码增量内容（delta） */
  codeDelta: string;
  /** 是否为流式输出的最后一个事件 */
  isFinal: boolean;
}
```

新增监听函数：

```typescript
/** 监听代码流式增量事件 */
export function onAgentCodeStreaming(
  handler: (payload: CodeStreamingPayload) => void,
): Promise<UnlistenFn> {
  return listen<CodeStreamingPayload>("agent:code_streaming", (event) => {
    handler(event.payload);
  });
}
```

#### 3.3.2 扩展 useAgent hook

在 `src/hooks/useAgent.ts` 中新增状态：

```typescript
// 新增状态
const [codeStreaming, setCodeStreaming] = useState<CodeStreamingPayload | null>(null);
```

在事件监听注册中新增：

```typescript
onAgentCodeStreaming((payload) => {
  // 后台会话：路由到缓存
  if (payload.sessionId !== sessionIdRef.current) {
    routeBackgroundEvent(payload.sessionId, {
      type: "code_streaming",
      callId: payload.callId,
      codeDelta: payload.codeDelta,
      isFinal: payload.isFinal,
    });
    return;
  }
  setCodeStreaming(payload);
}),
```

在 `BackgroundAgentEvent` 联合类型中新增：

```typescript
| { type: "code_streaming"; callId: string; codeDelta: string; isFinal: boolean }
```

在 `sendMessage` 中重置状态：

```typescript
setCodeStreaming(null);
```

在 `reset` 中重置状态：

```typescript
setCodeStreaming(initialState.codeStreaming);
```

在返回值中新增 `codeStreaming`。

#### 3.3.3 扩展 ToolNodeData 类型

在 `src/types/workflow.ts` 中扩展 `ToolNodeData`：

```typescript
export interface ToolNodeData {
  toolName: string;
  briefDescription: string;
  input: Record<string, unknown>;
  callId?: string;
  success?: boolean;
  error?: string;
  /** 流式代码内容（仅 code_interpreter_handler 时使用） */
  streamingCode?: string;
  /** 代码是否正在流式输出中 */
  isCodeStreaming?: boolean;
}
```

### 3.4 前端 Workflow Store 改动

#### 3.4.1 后台缓存处理

在 `useWorkflowStore.ts` 的 `applyBackgroundEvent` 方法中新增 `code_streaming` 分支：

```typescript
case "code_streaming": {
  // 通过 callId 匹配工具节点
  const toolNode = event.callId
    ? nodes.find((n) => n.type === "tool" && (n.data as { callId?: string }).callId === event.callId)
    : undefined;
  if (toolNode) {
    const existingCode = (toolNode.data as { streamingCode?: string }).streamingCode ?? "";
    nodes = nodes.map((n) =>
      n.id === toolNode.id
        ? {
            ...n,
            data: {
              ...n.data,
              streamingCode: existingCode + event.codeDelta,
              isCodeStreaming: !event.isFinal,
            },
          }
        : n
    );
  }
  break;
}
```

### 3.5 前端 App.tsx 改动

#### 3.5.1 处理 codeStreaming 事件

在 `App.tsx` 中新增 `useEffect` 处理 `codeStreaming` 状态变化：

```typescript
useEffect(() => {
  if (codeStreaming) {
    // 通过 callId 匹配已有的 ToolNode
    const toolNode = codeStreaming.callId
      ? useWorkflowStore.getState().nodes.find(
          (n) => n.type === "tool" && (n.data as ToolNodeData).callId === codeStreaming.callId
        )
      : undefined;

    if (toolNode) {
      const existingData = toolNode.data as ToolNodeData;
      const existingCode = existingData.streamingCode ?? "";
      updateNode(toolNode.id, {
        data: {
          ...existingData,
          streamingCode: existingCode + codeStreaming.codeDelta,
          isCodeStreaming: !codeStreaming.isFinal,
        },
      });
    }
  }
}, [codeStreaming, updateNode]);
```

**注意**：当 `codeStreaming.codeDelta` 为空且 `isFinal` 为 true 时，仅更新 `isCodeStreaming` 为 false，不追加空字符串。

#### 3.5.2 移除 ConfirmNode 对 code_interpreter_handler 的处理

由于 `code_interpreter_handler` 不再需要确认，`agent:confirm` 事件将不再为该 Handler 发射。但为了健壮性，在前端也做防御性处理：

```typescript
// App.tsx 中 pendingConfirmation 处理逻辑
useEffect(() => {
  if (pendingConfirmation) {
    // 防御性检查：code_interpreter_handler 不应再触发确认流程
    // 如果仍然收到，直接自动确认（兼容性兜底）
    if (pendingConfirmation.operationType === "code_interpreter_handler") {
      confirmOperation(pendingConfirmation.operationId, true);
      return;
    }

    // ... 其余确认逻辑保持不变（仅 delete_file 等其他高风险操作）...
  }
}, [pendingConfirmation, addNode, updateNode, confirmOperation, setConfirmHandler]);
```

### 3.6 前端 ToolNode 组件改动

#### 3.6.1 整体结构改造

将 ToolNode 从单行简要信息改为包含代码预览区域的复合组件：

```
+-- ToolNode (code_interpreter_handler, running) -----------------------+
| * [旋转图标] code_interpreter_handler . 执行代码 生成Word文档         |
| +-- 代码预览 ----------------------------------------------------+   |
| | import docx                                                    |   |
| | from docx import Document                                      |   |
| |                                                                |   |
| | doc = Document()                                               |   |
| | doc.add_heading('标题', level=1)                               |   |
| | doc.add_paragraph('内容...|')  <-- 流式光标                    |   |
| +----------------------------------------------------------------+   |
+-----------------------------------------------------------------------+

+-- ToolNode (code_interpreter_handler, completed) ---------------------+
| * [工具图标] code_interpreter_handler . 执行代码 生成Word文档         |
| +-- 代码预览 ------------------------------ > 展开代码 ----------+   |
| | import docx from docx import Document doc = Docu...            |   |
| +----------------------------------------------------------------+   |
+-----------------------------------------------------------------------+
```

#### 3.6.2 详细实现

```tsx
// src/components/workflow/ToolNode.tsx
import { useState, useEffect, useRef } from "react";
import type { WorkflowNode, ToolNodeData } from "../../types";
import { useTranslation } from 'react-i18next';
import { Icon } from "../common/Icon";

interface ToolNodeProps {
  node: WorkflowNode<"tool">;
}

export function ToolNode({ node }: ToolNodeProps) {
  const { t } = useTranslation();
  const data = node.data as ToolNodeData;
  const hasError = data.success === false;
  const isRunning = node.status === "running";
  const isCodeInterpreter = data.toolName === "code_interpreter_handler";
  const [errorExpanded, setErrorExpanded] = useState(false);

  // 代码预览展开/收缩状态
  // 初始展开：代码正在流式输出时展开，完成后收缩
  const [codeExpanded, setCodeExpanded] = useState(true);
  const prevIsCodeStreamingRef = useRef<boolean | undefined>(undefined);

  // 当代码流式输出结束时，自动收缩代码预览
  useEffect(() => {
    if (prevIsCodeStreamingRef.current === true && !data.isCodeStreaming) {
      setCodeExpanded(false);
    }
    prevIsCodeStreamingRef.current = data.isCodeStreaming;
  }, [data.isCodeStreaming]);

  // 错误信息截断
  const errorText = data.error || "";
  const shouldTruncateError = isCodeInterpreter && errorText.length > 150;
  const displayError = shouldTruncateError && !errorExpanded
    ? errorText.slice(0, 150) + "..."
    : errorText;

  // 代码内容
  const codeContent = data.streamingCode
    || (data.input?.code as string | undefined)
    || "";
  const isCodeStreaming = data.isCodeStreaming ?? false;

  // 收缩状态下显示的截断代码
  const collapsedCodePreview = codeContent.length > 80
    ? codeContent.slice(0, 80).split('\n')[0] + "..."
    : codeContent;

  return (
    <div className={`wf-node animate-node-in${isRunning ? " wf-tool-running" : ""}`}>
      <div className={`wf-node-dot${isRunning ? " wf-tool-dot-running" : " bg-bg-sub text-text-secondary"}`}>
        {isRunning ? (
          <svg className="wf-tool-spinner" viewBox="0 0 24 24" fill="none">
            <circle className="wf-tool-spinner-track" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
            <path className="wf-tool-spinner-arc" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
          </svg>
        ) : hasError ? (
          <Icon name="error" size={12} />
        ) : (
          <Icon name="tool" size={12} />
        )}
      </div>

      <div className="wf-tool-content">
        {/* 工具名称和简要描述 */}
        <div className="wf-tool-brief">
          <span className="font-mono">{data.toolName}</span>
          <span> · </span>
          <span>{data.briefDescription}</span>
          {isRunning && (
            <span className="wf-tool-status-running">{t('toolNode.executing')}</span>
          )}
          {hasError && data.error && (
            <span className="wf-tool-error">
              {" — "}
              {isCodeInterpreter ? t('toolNode.codeExecutionFailed') + ": " : ""}
              {displayError}
              {shouldTruncateError && (
                <button
                  className="wf-error-expand-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    setErrorExpanded(!errorExpanded);
                  }}
                >
                  {errorExpanded ? t('toolNode.collapseError') : t('toolNode.expandError')}
                </button>
              )}
            </span>
          )}
        </div>

        {/* 代码预览区域（仅 code_interpreter_handler 显示） */}
        {isCodeInterpreter && codeContent && (
          <div className={`wf-code-preview ${codeExpanded ? "wf-code-preview-expanded" : "wf-code-preview-collapsed"}`}>
            <div className="wf-code-preview-header">
              <span className="wf-code-preview-label">
                {isCodeStreaming ? t('toolNode.writingCode') : t('toolNode.codePreview')}
              </span>
              {!isCodeStreaming && (
                <button
                  className="wf-code-preview-toggle"
                  onClick={(e) => {
                    e.stopPropagation();
                    setCodeExpanded(!codeExpanded);
                  }}
                >
                  {codeExpanded ? t('toolNode.collapseCode') : t('toolNode.expandCode')}
                </button>
              )}
            </div>
            {codeExpanded ? (
              <pre className="wf-code-preview-content">
                {codeContent}
                {isCodeStreaming && <span className="wf-code-cursor" />}
              </pre>
            ) : (
              <div className="wf-code-preview-collapsed-text">
                {collapsedCodePreview}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
```

### 3.7 CSS 样式改动

在 `src/styles/globals.css` 中新增代码预览相关样式：

```css
/* ===== 代码预览卡片样式 ===== */

/* 工具节点内容容器 */
.wf-tool-content {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 代码预览区域 */
.wf-code-preview {
  border: 1px solid var(--color-border-light);
  border-radius: 6px;
  overflow: hidden;
  transition: all 0.3s ease;
}

/* 代码预览头部 */
.wf-code-preview-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px;
  background: var(--color-bg-sub);
  border-bottom: 1px solid var(--color-border-light);
}

.wf-code-preview-label {
  font-size: 11px;
  font-weight: 500;
  color: var(--color-text-tertiary);
}

.wf-code-preview-toggle {
  font-size: 10px;
  color: var(--color-accent);
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
}

.wf-code-preview-toggle:hover {
  text-decoration: underline;
}

/* 展开状态 - 代码内容区域 */
.wf-code-preview-content {
  margin: 0;
  padding: 8px 10px;
  font-family: 'Cascadia Code', 'Fira Code', 'Consolas', monospace;
  font-size: 12px;
  line-height: 1.6;
  color: var(--color-text-secondary);
  background: var(--color-bg);
  max-height: 400px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

/* 收缩状态 - 截断代码文本 */
.wf-code-preview-collapsed-text {
  padding: 4px 10px;
  font-family: 'Cascadia Code', 'Fira Code', 'Consolas', monospace;
  font-size: 11px;
  line-height: 1.4;
  color: var(--color-text-quaternary);
  background: var(--color-bg);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  /* 收缩状态高度约 28px，与展开状态的 200-400px 形成明显对比 */
  height: 28px;
  display: flex;
  align-items: center;
}

/* 流式光标 */
.wf-code-cursor {
  display: inline-block;
  width: 2px;
  height: 14px;
  background: var(--color-accent);
  margin-left: 1px;
  vertical-align: middle;
  animation: blink 1s step-end infinite;
}
```

**高度差异说明**：
- **展开状态**：代码内容区域 `max-height: 400px`，由内容自然撑开，通常 200-400px
- **收缩状态**：截断文本区域 `height: 28px`，仅显示一行截断代码
- 高度差比约为 **7:1 ~ 14:1**，视觉对比非常明显

### 3.8 ConfirmNode 改动

由于 `code_interpreter_handler` 不再需要确认，ConfirmNode 的改动如下：

1. **ConfirmNode 不再为 `code_interpreter_handler` 弹出**：后端不再发射 `agent:confirm` 事件，前端不再创建 ConfirmNode
2. **ConfirmNode 组件本身保留**：其他高风险操作（如 `delete_file`）仍可能需要确认，ConfirmNode 组件和逻辑保留
3. **ConfirmNode 中的代码预览区域保留**：虽然 `code_interpreter_handler` 不再触发确认，但其他场景可能需要代码预览，保留不影响
4. **前端防御性处理**：如果意外收到 `code_interpreter_handler` 的确认请求，自动批准执行（见 3.5.2）

### 3.9 设置页面改动

确认级别设置（`GeneralTab` 中的 `confirmationLevel` 下拉选择）**保留不变**。该设置仍对 `delete_file` 等其他高风险操作生效。`code_interpreter_handler` 在任何级别下都不需要确认，这是硬编码行为，不受用户设置影响。

---

## 4. 事件时序图（改动后）

```
LLM 流式输出 tool_call 参数
  |
  +-- 检测到 code_interpreter_handler 名称
  |   -> emit agent:tool_call (参数可能不完整)
  |   -> emit agent:code_streaming (code_delta: "import docx\n")
  |
  +-- 继续流式输出 code 参数
  |   -> emit agent:code_streaming (code_delta: "from docx import Document\n")
  |   -> emit agent:code_streaming (code_delta: "doc = Document()\n")
  |   -> ... (持续发射)
  |
  +-- 流式结束
  |   -> emit agent:tool_call (参数完整，前端通过 callId 去重更新)
  |   -> emit agent:code_streaming (code_delta: "", is_final: true)
  |
  +-- 自动执行代码（无需确认）
  |
  +-- 执行完成 -> emit agent:tool_result
      ToolNode 代码预览自动收缩
```

**与原流程的关键差异**：
- 移除了 `agent:confirm` 事件
- 移除了用户确认/拒绝的交互步骤
- 代码编写完成后直接执行，无需等待

---

## 5. 边界情况与异常处理

### 5.1 JSON 解析失败

LLM 流式输出 `arguments` 时，中间状态的 JSON 可能不完整，导致无法提取 `code` 字段。

**处理策略**：解析失败时不发射 `agent:code_streaming` 事件，等待下次增量。前端 ToolNode 在未收到任何 `code_streaming` 事件时，不显示代码预览区域（降级为原有行为）。

### 5.2 流式中断/网络错误

如果流式响应中断，`code_streaming` 事件停止发射，但 `is_final` 事件未到达。

**处理策略**：
- 前端 ToolNode 的 `isCodeStreaming` 状态由 `agent:code_streaming` 的 `isFinal` 字段控制
- 当 `agent:tool_result` 到达时，强制将 `isCodeStreaming` 设为 false，触发收缩
- 当 `agent:error` 或 `agent:stopped` 到达时，同样强制关闭代码流式状态

### 5.3 截断重试

当 LLM 响应被截断时，后端会回滚并重试。此时可能已发射了部分 `code_streaming` 事件。

**处理策略**：
- 截断重试时，后端在发射截断关闭事件（`agent:tool_result` with `success: false`）之前，先发射 `agent:code_streaming` with `is_final: true`
- 前端收到 `agent:tool_result` with `success: false` 后，清除 `streamingCode` 内容
- 重试时重新发射 `agent:code_streaming` 事件，前端重新累积代码内容

### 5.4 多个 code_interpreter_handler 同时调用

LLM 可能在一次响应中调用多个 `code_interpreter_handler`。

**处理策略**：每个 `code_streaming` 事件都携带 `callId`，前端通过 `callId` 精确匹配对应的 ToolNode，互不干扰。

### 5.5 后台会话

当用户切换到其他会话时，后台会话的 `code_streaming` 事件需要正确路由到缓存。

**处理策略**：与现有 `tool_call`/`tool_result` 事件相同，通过 `sessionId` 判断是否为当前会话，非当前会话路由到 `applyBackgroundEvent`。

### 5.6 历史会话恢复

从历史消息加载时，`ToolNodeData` 中已有 `input.code` 字段（完整代码），但无 `streamingCode` 和 `isCodeStreaming` 字段。

**处理策略**：
- `loadFromMessages` 创建 ToolNode 时，不设置 `streamingCode` 和 `isCodeStreaming`
- ToolNode 组件中，当 `streamingCode` 不存在时，回退到 `input.code` 作为代码内容
- 历史节点的代码预览默认为收缩状态（因为 `isCodeStreaming` 为 undefined/false）

### 5.7 代码执行失败

代码自动执行后可能失败（语法错误、运行时错误等）。

**处理策略**：
- 代码执行失败时，后端发射 `agent:tool_result` with `success: false` 和错误信息
- 前端 ToolNode 显示错误状态和错误信息
- Agent 会根据错误信息自动重试或调整代码
- 代码预览卡片仍然收缩，用户可手动展开查看完整代码

### 5.8 确认级别为 Always 时的行为

当用户将确认级别设为 `Always` 时，其他 Handler/Tool 仍需确认，但 `code_interpreter_handler` 始终跳过确认。

**处理策略**：在 `needs_confirmation` 方法中显式排除 `code_interpreter_handler`，无论确认级别如何，均返回 `false`。

---

## 6. 国际化

新增翻译键：

```json
// zh-CN.json
{
  "toolNode": {
    "writingCode": "正在编写代码...",
    "codePreview": "代码预览",
    "collapseCode": "收缩代码",
    "expandCode": "展开代码",
    "executing": "执行中",
    "codeExecutionFailed": "代码执行失败"
  }
}

// en-US.json
{
  "toolNode": {
    "writingCode": "Writing code...",
    "codePreview": "Code Preview",
    "collapseCode": "Collapse",
    "expandCode": "Expand",
    "executing": "Executing",
    "codeExecutionFailed": "Code execution failed"
  }
}
```

---

## 7. 改动文件清单

| 文件路径 | 改动类型 | 说明 |
|---------|---------|------|
| `src-tauri/src/events/types.rs` | 修改 | 新增 `CodeStreamingPayload` 结构体和 `AGENT_CODE_STREAMING` 常量 |
| `src-tauri/src/events/emitter.rs` | 修改 | 新增 `emit_code_streaming` 方法 |
| `src-tauri/src/services/agent/executor.rs` | 修改 | (1) 流式收集 `code_interpreter_handler` 参数时发射 `code_streaming` 事件；(2) 从 `HIGH_RISK_HANDLERS` 移除 `code_interpreter_handler`；(3) `needs_confirmation` 显式排除 `code_interpreter_handler`；(4) 移除 `request_confirmation` 中 `code_interpreter_handler` 分支 |
| `src/services/event.ts` | 修改 | 新增 `CodeStreamingPayload` 类型和 `onAgentCodeStreaming` 监听函数 |
| `src/hooks/useAgent.ts` | 修改 | 新增 `codeStreaming` 状态和事件监听 |
| `src/types/workflow.ts` | 修改 | `ToolNodeData` 新增 `streamingCode` 和 `isCodeStreaming` 字段 |
| `src/stores/useWorkflowStore.ts` | 修改 | `BackgroundAgentEvent` 新增 `code_streaming` 类型，`applyBackgroundEvent` 新增处理分支 |
| `src/App.tsx` | 修改 | (1) 新增 `codeStreaming` 状态的 useEffect 处理；(2) `pendingConfirmation` 防御性自动确认 `code_interpreter_handler` |
| `src/components/workflow/ToolNode.tsx` | 修改 | 新增代码预览区域，支持展开/收缩/流式显示 |
| `src/styles/globals.css` | 修改 | 新增代码预览卡片相关样式 |
| `src/i18n/locales/zh-CN.json` | 修改 | 新增翻译键 |
| `src/i18n/locales/en-US.json` | 修改 | 新增翻译键 |

**不需要改动的文件**：
- `src/components/workflow/ConfirmNode.tsx`：组件保留，其他高风险操作仍需确认
- `src/components/settings/GeneralTab.tsx`：确认级别设置保留，仍对其他操作生效
- `src-tauri/src/config/app_settings.rs`：`ConfirmationLevel` 枚举保留
- `src-tauri/src/commands/agent.rs`：`confirm_operation` 命令保留

---

## 8. 测试要点

### 8.1 功能测试

1. **流式显示**：触发 `code_interpreter_handler` 调用，验证代码在 ToolNode 中实时流式显示
2. **初始展开**：验证代码预览卡片在代码开始编写时处于展开状态
3. **自动收缩**：验证代码编写完成后，代码预览卡片自动切换为收缩状态
4. **手动展开/收缩**：验证用户可以手动点击展开/收缩按钮切换代码预览状态
5. **高度差异**：验证展开状态和收缩状态的代码预览框有明显的高度差异
6. **流式光标**：验证代码流式输出时显示闪烁光标
7. **自动执行**：验证代码编写完成后自动执行，无需用户确认
8. **无 ConfirmNode**：验证 `code_interpreter_handler` 调用时不弹出 ConfirmNode
9. **其他操作确认正常**：验证 `delete_file` 等其他高风险操作仍需确认（在 EditOnly 级别下）

### 8.2 异常测试

1. **网络中断**：验证流式中断时代码预览状态的正确处理
2. **截断重试**：验证 LLM 响应截断重试时，代码预览内容的正确重置
3. **用户停止**：验证用户手动停止 Agent 时，代码预览状态的正确处理
4. **多工具调用**：验证多个 `code_interpreter_handler` 同时调用时，代码预览互不干扰
5. **代码执行失败**：验证代码自动执行失败时，ToolNode 正确显示错误信息

### 8.3 兼容性测试

1. **历史会话**：验证加载历史会话时，ToolNode 代码预览正确显示（收缩状态，使用 `input.code`）
2. **后台会话**：验证切换会话后，后台会话的代码流式事件正确路由到缓存
3. **非 code_interpreter_handler**：验证其他 Handler 调用时，ToolNode 不显示代码预览区域
4. **确认级别 Always**：验证确认级别设为 Always 时，`code_interpreter_handler` 仍自动执行，其他操作需确认
5. **确认级别 Never**：验证确认级别设为 Never 时，所有操作均自动执行

---

## 9. 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| JSON 增量解析失败导致 code_streaming 事件丢失 | 代码预览不显示或显示不完整 | 降级为原有行为（无代码预览），`agent:tool_call` 携带完整参数时仍可回退显示 |
| 高频 code_streaming 事件导致前端性能问题 | UI 卡顿 | 事件标记为高频流式（`high_frequency: true`），日志级别为 TRACE；前端使用 React 状态批量更新 |
| 流式光标动画影响虚拟滚动性能 | 滚动不流畅 | 光标使用 CSS animation，不触发 React 重渲染 |
| 代码内容过长导致展开状态占用过多空间 | 页面布局被撑开 | 展开状态设置 `max-height: 400px` + `overflow: auto` |
| 代码自动执行可能运行危险代码 | 安全风险 | (1) Python Sidecar 运行在沙箱环境中，已有超时和资源限制；(2) 代码执行在工作区内，已有路径安全校验；(3) 版本快照机制保护文件可回滚 |
| 移除确认后用户无法阻止代码执行 | 用户体验 | 用户仍可通过"停止 Agent"按钮中断执行；版本快照可回滚文件变更 |

---

## 10. 确认机制移除的详细影响分析

### 10.1 被移除的功能

| 功能 | 移除原因 | 替代方案 |
|------|---------|---------|
| `code_interpreter_handler` 的 ConfirmNode 弹出 | 代码自动执行，无需确认 | ToolNode 中的代码预览卡片替代 |
| ConfirmNode 中的代码预览区域 | 不再弹出 ConfirmNode | ToolNode 中的流式代码预览替代 |
| `agent:confirm` 事件（仅针对 code_interpreter_handler） | 不再需要确认 | 无需替代 |
| `confirm_operation` 命令（仅针对 code_interpreter_handler） | 不再需要确认 | 无需替代 |
| `tool_name: "code_interpreter_handler (等待确认)"` 标记 | 不再需要确认 | ToolNode 直接显示工具名称 |
| `request_confirmation` 中的代码摘要拼接 | 不再需要确认 | ToolNode 代码预览实时显示完整代码 |

### 10.2 保留的功能

| 功能 | 保留原因 |
|------|---------|
| `ConfirmNode` 组件 | 其他高风险操作（如 `delete_file`）仍需确认 |
| `confirm_operation` 命令 | 其他高风险操作仍需确认 |
| `ConfirmationLevel` 枚举和设置 | 用户仍可控制其他操作的确认行为 |
| `confirm_channels` 机制 | 其他高风险操作的确认通道仍需使用 |
| `HIGH_RISK_HANDLERS` 列表 | `delete_file` 仍为高风险操作 |
| `request_confirmation` 方法 | 其他高风险操作仍需调用 |

### 10.3 向后兼容性

- **历史会话**：历史会话中可能包含 `code_interpreter_handler` 的 ConfirmNode 记录。加载时 ConfirmNode 会正常渲染（显示"已确认"状态），不影响历史数据查看
- **配置文件**：`confirmationLevel` 设置保持不变，用户无需重新配置
- **数据库**：历史消息中的确认相关记录（tool_name 含"等待确认"标记）保持不变，仅影响新消息的生成
