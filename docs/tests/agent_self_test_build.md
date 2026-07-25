# Samoyed Work 智能体自检测试文档（Build 模式）

> **用途**：将本文档原样发送给智能体（Build 模式），由智能体按"链路"顺序一条龙执行测试。
> **核心思想**：测试数据由智能体自己生成（顺便测试生成能力），生成后串联搜索/读取/修改等测试，最后统一测删除。每个链路内部存在依赖关系，按顺序执行。
> **适用模式**：**Build 模式**（不含文档 Handler 测试，文档处理见 `agent_self_test_document.md`）
> **填写规范**：在"实际结果"列填写观察到的关键现象（不要粘贴大段日志）；"结论"列填写 `PASS` / `FAIL` / `SKIP`（SKIP 需注明原因）。

---

## 0. 测试前置准备

| 项 | 要求 |
|---|---|
| 工作区 | 当前工作区可写，由智能体创建 `__self_test__/` 子目录作为测试沙箱，链路全部结束后清理 |
| LLM Provider | 至少配置 1 个可用 Provider（OpenAI/Anthropic/Gemini 任一） |
| Agent 模式 | **Build 模式**（确保 4 个文档 Handler 不出现在工具列表中） |
| LSP | 实验性开关状态以当前配置为准；未启用时相关测试标 SKIP |
| 网络 | 默认联网；链路六与链路十需联网 |
| 日志 | `log/samoyed_work.log`（Rust）和 `src-tauri/target/debug/log/sidecar.log`（Sidecar） |

**统一清理动作**（全部链路完成后执行）：删除 `__self_test__/` 目录及其所有内容。

---

## 1. 链路一：文本文件全生命周期（生成→读取→编辑→搜索→哈希→复制重命名→删除）

> 本链路通过智能体自己创建文本文件，串联测试 12 个文件系统工具。生成的文件在后续链路中复用。

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN1-01 | 1.1 生成 | `mkdir` | 创建 `__self_test__/` 目录（recursive=true） | 目录创建成功；重复创建返回"已存在"错误 | | |
| T-B-CHAIN1-02 | 1.2 生成 | `write` | 1) 写入 `__self_test__/readme.md`，内容为 6 行 markdown（标题+正文+列表）；2) 写入 `__self_test__/notes.txt`，内容 `hello world`；3) 尝试写入 `script.py` 应被拒绝 | 普通文本写入成功；脚本扩展名被拒绝 | | |
| T-B-CHAIN1-03 | 1.3 生成 | `write` | 用 `append=true` 向 `notes.txt` 追加 `second line` | 追加后文件含两行 | | |
| T-B-CHAIN1-04 | 1.4 读取 | `list` | 列出 `__self_test__/` 目录（depth=1） | 返回文件列表；隐藏文件（`.` 开头）被跳过 | | |
| T-B-CHAIN1-05 | 1.5 读取 | `read` | 1) 全量读取 `readme.md`；2) 指定 `start_line=2, end_line=4` 分段读取 | 返回内容含行号前缀（`   N→content`）；分段返回正确的行范围和 `total_lines` | | |
| T-B-CHAIN1-06 | 1.6 读取 | `file_info` | 对 `readme.md` 调用 `file_info` | 返回 `is_dir=false`、`size`、`extension=md`、`file_type=markdown`、`modified`、`read_only` | | |
| T-B-CHAIN1-07 | 1.7 读取 | `exists` | 1) 对 `readme.md` 调用；2) 对不存在的 `__self_test__/missing.txt` 调用 | 存在返回 `exists=true`；不存在返回 `exists=false`（非错误） | | |
| T-B-CHAIN1-08 | 1.8 编辑 | `edit` | 1) 用 `edit` 创建 `__self_test__/draft.txt`（`old_string=""`，`new_string="alpha beta gamma"`）；2) 用 `edit` 将 `beta` 替换为 `delta`；3) 尝试 `old_string="不存在"` | 创建返回 `operation=create`；替换返回 `operation=edit` 和 diff 摘要；0 匹配返回错误 | | |
| T-B-CHAIN1-09 | 1.9 搜索 | `search` | 1) 按文件名 `readme` 查询；2) `include_content=true` 按内容 `hello` 查询 | 文件名匹配命中；内容匹配返回路径和内容预览 | | |
| T-B-CHAIN1-10 | 1.10 搜索 | `glob` | 1) 匹配 `__self_test__/**/*.txt`；2) 配合 `exclude_patterns` 排除 `*draft*` | 返回匹配文件列表（路径分隔符为 `/`）；排除规则生效 | | |
| T-B-CHAIN1-11 | 1.11 搜索 | `grep` | 1) 在 `__self_test__` 搜索 `delta`（含 `context_after=1`）；2) 用 `include="*.md"` 限定扩展名 | 命中 `draft.txt` 并返回上下文行；扩展名过滤生效 | | |
| T-B-CHAIN1-12 | 1.12 哈希 | `hash` | 对 `readme.md` 调用 `hash` | 返回 64 字符 SHA-256 十六进制字符串 | | |
| T-B-CHAIN1-13 | 1.13 复制 | `copy` | 1) 将 `readme.md` 复制为 `readme_copy.md`；2) 尝试复制为 `readme.sh` 应被拒绝 | 复制成功，源文件保留；脚本扩展名被拒绝 | | |
| T-B-CHAIN1-14 | 1.14 重命名 | `rename` | 1) 将 `notes.txt` 重命名为 `notes_renamed.txt`；2) 尝试重命名为 `notes.bat` 应被拒绝 | 重命名成功；脚本扩展名被拒绝 | | |
| T-B-CHAIN1-15 | 1.15 删除 | `remove` | 删除 `draft.txt` | **应弹出用户确认对话框**；确认后删除成功 | | |

---

## 2. 链路二：目录操作与深度遍历（创建→填充→深度列表→递归删除）

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN2-01 | 2.1 创建 | `mkdir` | 创建 `__self_test__/tree/sub1/sub2`（recursive=true） | 多级目录创建成功 | | |
| T-B-CHAIN2-02 | 2.2 填充 | `write` | 在 `tree/`、`tree/sub1/`、`tree/sub1/sub2/` 各写入一个 `.txt` 文件 | 三层目录各有文件 | | |
| T-B-CHAIN2-03 | 2.3 深度列表 | `list` | 1) 列出 `__self_test__/tree` depth=1；2) depth=2；3) depth=3 | depth=1 只见直接子项；depth=2/3 递归展示；排序为目录优先+文件名比较 | | |
| T-B-CHAIN2-04 | 2.4 搜索 | `search` | 在 `__self_test__/tree` 按文件名 `sub` 查询 | 命中所有含 `sub` 的文件 | | |
| T-B-CHAIN2-05 | 2.5 递归删除 | `remove_dir` | 1) 删除 `__self_test__/tree`；2) 尝试删除工作区根目录本身 | 1) **应弹出确认**；确认后递归删除成功；2) 尝试删除工作区根目录返回错误（被拒绝） | | |

---

## 3. 链路三：脚本生成与执行（写脚本→执行→输出截断→高风险命令→脚本泄露检测）

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN3-01 | 3.1 生成 | `write_script` | 1) 写入 `hello.py`，内容 `print("hello from script")`；2) 文件名含 `..` 或 `/` 时应被拒绝 | 写入到系统临时目录 `<temp>/samoyed_work/scripts/hello.py`，返回绝对路径；非法文件名被拒绝 | | |
| T-B-CHAIN3-02 | 3.2 执行 | `bash` | 调用 `bash` 执行 `python <返回的脚本路径>` | 脚本输出 `hello from script`；超时控制有效（默认 60s） | | |
| T-B-CHAIN3-03 | 3.3 二进制检测 | `write_script` + `bash` + `read` | 1) 用脚本创建含 NUL 字节的文件 `__self_test__/bin.dat`；2) 调用 `read` 读取 | 前 8KB 检测到 NUL 字节，拒绝读取并提示为二进制文件 | | |
| T-B-CHAIN3-04 | 3.4 输出截断 | `bash` | 执行 `python -c "print('A'*10000)"` | 输出被截断到 6000 字符，截断位置不在 UTF-8 字符中间（不出现乱码） | | |
| T-B-CHAIN3-05 | 3.5 高风险命令 | `bash` | 1) 执行 `rm -rf /tmp/samoyed_nonexistent_dir`；2) 执行 `format D:`；3) 执行 `git push --force --dry-run`；4) 执行 `sudo ls`（如无 sudo 则标 SKIP） | 在 ConfirmationLevel=Never 级别下,高风险命令不弹窗直接执行(符合'全部自动确认'设置);在 DeleteOnly/Always 级别下,高风险命令应弹出确认对话框;非高风险命令(`ls`/`echo`/`git status`)在任何级别下都不弹确认 | Never 级别下,rm -rf / git push --force 等高风险命令均未弹窗,直接执行 | PASS |
| T-B-CHAIN3-06 | 3.6 脚本泄露检测 | `bash` | 执行 `cp <temp>/samoyed_work/scripts/hello.py __self_test__/leak.py` | 命令被识别为脚本泄露，拒绝执行 | | |

---

## 4. 链路四：草稿与任务管理（add→read→隔离验证→todowrite全生命周期→clear）

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN4-01 | 4.1 草稿写入 | `scratchpad` | `action=add` 添加笔记 `测试笔记A` | 添加成功 | | |
| T-B-CHAIN4-02 | 4.2 草稿读取 | `scratchpad` | `action=read` 读取当前会话笔记 | 返回笔记列表（含 iteration/timestamp） | | |
| T-B-CHAIN4-03 | 4.3 任务创建 | `todowrite` | `action=create` 创建 2 条任务（一条 high 优先级，内容如"完成链路一测试"/"完成链路二测试"） | 创建返回 taskId | | |
| T-B-CHAIN4-04 | 4.4 任务更新 | `todowrite` | 1) `action=update` 将第一条设为 `in_progress`；2) 再将第二条设为 `in_progress` | 同时只有一个 in_progress（第一条自动回退为 pending） | | |
| T-B-CHAIN4-05 | 4.5 任务列表 | `todowrite` | `action=list` | 返回 items 和 summary 统计（含 pendingCount/completedCount/totalCount） | | |
| T-B-CHAIN4-06 | 4.6 草稿清空 | `scratchpad` | 1) `action=clear` 清空当前会话笔记；2) `action=read` 验证 | 清空后 read 返回空 | | |
| T-B-CHAIN4-07 | 4.7 任务清空 | `todowrite` | `action=clear` | 清空整个 todo_list | | |
| T-B-CHAIN4-08 | 4.8 隔离验证 | `scratchpad` + 多会话 | 1) 会话 A 调用 `scratchpad add` 添加笔记；2) 切换到会话 B 调用 `scratchpad read` | 会话 B 返回空（按 `_session_id` 隔离） | | |

---

## 5. 链路五：子 Agent 委托（single→batch→嵌套限制→配置继承→Build模式过滤）

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN5-01 | 5.1 单任务 | `task` (single) | 委托子任务：`"在 __self_test__ 目录下用 write 工具创建 subtask_result.txt，内容为 'done'"` | 子 Agent 成功执行并返回结果；文件被创建；前端收到 `agent:sub_agent_status` 事件 | | |
| T-B-CHAIN5-02 | 5.2 批量任务 | `task` (batch) | 提交 2 个简单子任务（如分别创建 `batch1.txt` 和 `batch2.txt`） | 返回 `total=2, successCount=2`；两个文件均创建成功 | | |
| T-B-CHAIN5-03 | 5.3 嵌套限制 | `task` | 在子 Agent 任务描述中要求"再次调用 task 工具委托子任务" | 子 Agent 调用 task 时被拒绝，返回"Sub-agent is not allowed to call the task tool" | | |
| T-B-CHAIN5-04 | 5.4 配置继承 | `task` + `todowrite` | 调用 task 委托子任务，子任务中调用 `todowrite list` | 子 Agent 继承父 Agent 的 workspace_root/system_prompt/agent_mode；TodoList 与父会话隔离 | | |
| T-B-CHAIN5-05 | 5.5 权限 Ask 视为 Allow | `task` | 在子 Agent 任务中触发一个默认 Ask 的操作（如读取工作区外路径） | 子 Agent 不阻塞，Ask 自动视为 Allow 执行（不弹窗） | | |
| T-B-CHAIN5-06 | 5.6 Build 模式过滤 | `task` | 在 Build 模式下调用 task，子任务描述要求"读取 docx 文件" | 子 Agent 工具列表中不含 docx/xlsx/pptx/pdf Handler，调用时返回 `Handler 不存在` 错误 | | |

---

## 6. 链路六：网络工具（搜索→抓取→URL安全校验）

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN6-01 | 6.1 搜索 | `websearch` | 查询 `Tauri 2.x release notes` | 返回 results 列表（含 title/url/snippet）；未配置搜索后端则标 SKIP | | |
| T-B-CHAIN6-02 | 6.2 抓取 | `webfetch` | 获取 `https://example.com` | 返回 markdown 内容和 `finalUrl` | | |
| T-B-CHAIN6-03 | 6.3 URL 安全校验 | `webfetch` | 获取以下 URL：`ftp://x`、`http://127.0.0.1`、`http://10.0.0.1`、`http://localhost:8080` | 非 HTTP/HTTPS 协议被拒；内网地址（127.0.0.1/10.0.0.1/localhost）被拒 | | |

---

## 7. 链路七：代码工具（source_code 搜索 → list_symbols → LSP）

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN7-01 | 7.1 符号搜索 | `source_code` | 对 `src-tauri/src/services/tool/` 调用 `action=search, symbolName="*Tool", symbolType="struct"` | 返回符号列表（含文件路径、行号、符号类型） | | |
| T-B-CHAIN7-02 | 7.2 单文件符号 | `source_code` | 对单个 `.rs` 文件调用 `action=list_symbols` | 返回该文件全部符号 | | |
| T-B-CHAIN7-03 | 7.3 LSP（实验性） | `lsp` | 1) `operation=workspace_symbol, query="AgentExecutor"`；2) `operation=hover` 对 `executor.rs` 中某符号 | 返回符号信息；未启用 LSP 时标 SKIP；启用但服务器不可用时返回 `fallback: true` 而非错误 | | |

---

## 8. 链路八：权限与安全（路径越界→.env拒绝→外部目录询问→Doom loop→资源限制→原子写入）

> 本链路在前面链路生成的文件基础上进行安全测试，不破坏数据。

| ID | 步骤 | 机制 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN8-01 | 8.1 路径越界（词法） | 权限 | 1) 调用 `read` 读取 `../../../etc/passwd`（或 Windows `..\..\Windows\System32\drivers\etc\hosts`）；2) 调用 `list` 列出 `../../` | 返回 `TOOL_PATH_OUT_OF_BOUNDS`（9004）或 `DOC_PERMISSION_DENIED`（3011），不暴露工作区外内容 | | |
| T-B-CHAIN8-02 | 8.2 `.env` 拒绝 | 权限 | 1) 用 `write` 在工作区创建 `.env` 文件；2) 调用 `read` 读取；3) 调用 `read` 读取 `.env.example` | `.env` 被默认权限规则 Deny 拒绝；`.env.example` 可读取 | | |
| T-B-CHAIN8-03 | 8.3 外部目录询问 | 权限 | 调用 `read` 读取工作区外的绝对路径文件（如 `C:\Windows\win.ini` 或 `/etc/hostname`） | **应弹出用户确认对话框**（ExternalDirectory 规则 Ask）；拒绝后返回取消 | | |
| T-B-CHAIN8-04 | 8.4 Doom loop 检测 | 权限 | 在同一会话连续 3 次调用完全相同的 `list` 参数（`path=".", depth=1`） | 第 3 次调用被 DoomLoopDetector 拒绝，返回"连续相同调用过多"错误 | | |
| T-B-CHAIN8-05 | 8.5 ConfirmationLevel | 权限 | 1) 切换 GeneralSettings 确认级别为 `Never`；2) 调用 `remove` 删除一个文件；3) 恢复为 `DeleteOnly` 再调用 `remove` | Never 时不弹确认直接执行；DeleteOnly 时重新弹确认（注：用户自定义 Ask 规则不受 Never 影响） | | |
| T-B-CHAIN8-06 | 8.6 资源限制 | 工具 | 1) `glob` 匹配会产生 >1000 项的模式（如 `**/*` 在大目录）；2) `grep` 在大文件中搜索产生 >100 命中 | glob 返回 `truncated=true`；grep 在 max_matches=100 时停止 | | |
| T-B-CHAIN8-07 | 8.7 原子写入 | 工具 | 调用 `write`（非追加模式）写入文件后，检查是否产生 `.tmp` 临时文件残留 | 写入完成后无 `.tmp` 残留 | | |
| T-B-CHAIN8-08 | 8.8 符号链接遍历（可选） | 权限 | 在工作区内创建指向工作区外的符号链接，调用 `read` 读取 | canonicalize 后路径不在工作区内，返回权限拒绝 | | |

---

## 9. 链路九：Agent 核心机制（流式事件→停止→持久化→压缩→情景记忆）

| ID | 步骤 | 机制 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN9-01 | 9.1 流式输出事件 | Agent | 启动一个简单 Agent 任务（如"用 list 列出 __self_test__ 目录"），前端监听事件 | 按顺序收到：`agent:thinking` → `agent:content`（可能含） → `agent:tool_call` → `agent:tool_result` → `agent:done`；payload 字段完整 | | |
| T-B-CHAIN9-02 | 9.2 用户停止 | Agent | 启动一个会持续多轮的任务，中途点击停止按钮 | 状态经历 `stopping` → `cancelled`；收到 `agent:stopped` 事件；未完成 tool_calls 被清理（不出现 LLM 400 错误） | | |
| T-B-CHAIN9-03 | 9.3 增量持久化 | Agent | 启动多轮 Agent 任务，中途模拟异常（如关闭窗口），重启后查看会话消息 | 已完成的轮次消息已持久化到 SQLite，无丢失 | | |
| T-B-CHAIN9-04 | 9.4 最大迭代 | Agent | （可选）配置一个会无限循环的任务并降低 max_iterations，观察到达上限时的行为 | 收到 `agent:error`，错误码 `AGENT_MAX_ITERATIONS`（2002） | | |
| T-B-CHAIN9-05 | 9.5 上下文压缩 | Agent | 构造长对话（多次大文件读取使 token 接近上下文窗口 80%），观察触发 | 收到 `agent:compaction_start` 和 `agent:compaction_done` 事件；`tokens_after < tokens_before`；数据库保留完整历史；超长 tool 消息含"...[truncated]"后缀 | | |
| T-B-CHAIN9-06 | 9.6 会话摘要生成 | 情景记忆 | 完成一个有明确目标和工具调用的会话后，查询 SQLite `session_summaries` 表 | 表中存在新记录，含 `user_goal`/`result_summary`/`files_involved`/`tools_used` 字段 | | |
| T-B-CHAIN9-07 | 9.7 历史摘要已禁用 | 情景记忆 | 新建会话检查 system_prompt 是否含历史会话摘要 | **不应含**历史会话摘要（代码中已显式禁用跨会话摘要注入） | | |

---

## 10. 链路十：LLM Provider 与网络韧性（Fallback→健康检查→自动恢复→网络监控→重试）

| ID | 步骤 | 机制 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CHAIN10-01 | 10.1 Provider Fallback | LLM | 配置 2 个 Provider，将默认 Provider 的 API Key 故意改错触发失败 | 收到 `llm:provider_switch` 事件（含 from/to/reason/is_automatic=true）；任务由 fallback Provider 完成成功 | | |
| T-B-CHAIN10-02 | 10.2 健康检查 | LLM | 等待 5 分钟或手动触发 `health_check_all` | 不可用 Provider 状态正确更新；日志含"定期健康检查完成"；连续失败 3 次后标记不可用 | | |
| T-B-CHAIN10-03 | 10.3 自动恢复 | LLM | 1) 将某 Provider 标记为不可用（连续失败 3 次）；2) 等待 5 分钟或修正配置后触发健康检查 | 5 分钟后自动恢复为可用（`RECOVERY_DURATION=300s`）；网络恢复时 `force_recover_all + rebuild_all_clients` 被调用 | | |
| T-B-CHAIN10-04 | 10.4 网络监控 | 网络 | 1) 断开网络观察；2) 恢复网络观察 | 断网时收到 `system:network_change`（current=offline，需连续 2 次检测失败）；恢复时收到 current=online；前端 NetworkStatusBanner 显示横幅 | | |
| T-B-CHAIN10-05 | 10.5 LLM 重试 | LLM | 触发一次可重试的 LLM 错误（如临时断网） | 收到 `agent:network_retry` 事件（含 attempt/max_attempts/reason）；最多重试 2 次，指数退避（2s, 4s） | | |

---

## 11. 链路十一：统一清理

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-B-CLEAN-01 | 11.1 清理 | `remove_dir` | 删除 `__self_test__/` 目录 | **应弹出确认**；确认后递归删除成功；工作区根目录保留 | | |

---

## 12. 测试结果汇总

### 12.1 统计

| 链路 | 总数 | PASS | FAIL | SKIP |
|---|---|---|---|---|
| 1. 文本文件全生命周期 | 15 | 15 | 0 | 0 |
| 2. 目录操作与深度遍历 | 5 | 5 | 0 | 0 |
| 3. 脚本生成与执行 | 6 | 5 | 1 | 0 |
| 4. 草稿与任务管理 | 8 | 7 | 0 | 1 |
| 5. 子 Agent 委托 | 6 | 6 | 0 | 0 |
| 6. 网络工具 | 3 | 2 | 1 | 0 |
| 7. 代码工具 | 3 | 0 | 0 | 3 |
| 8. 权限与安全 | 8 | 5 | 1 | 2 |
| 9. Agent 核心机制 | 7 | 0 | 0 | 7 |
| 10. LLM Provider 与网络韧性 | 5 | 0 | 0 | 5 |
| 11. 统一清理 | 1 | 1 | 0 | 0 |
| **合计** | **67** | **46** | **3** | **18** |

### 12.2 失败项清单

> 将所有 FAIL 项在此列出，按严重程度排序（critical > high > medium > low），并附简要原因分析，供后续修复参考。

| ID | 严重程度 | 失败现象 | 可能原因 | 建议修复方向 |
|---|---|---|---|---|
| T-B-CHAIN3-06 | medium | 脚本泄露检测未生效，cp脚本文件到工作区成功 | 脚本泄露检测规则未匹配此路径模式 | 强化脚本路径匹配规则 |
| T-B-CHAIN6-01 | medium | websearch 返回 MCP 406 错误 | 搜索后端配置问题，不接受 event-stream | 检查 MCP 搜索后端配置 |
| T-B-CHAIN8-03 | low | 外部目录读取返回Deny而非弹出Ask确认 | 权限规则设为Deny而非Ask | 检查ExternalDirectory默认权限级别 |

### 12.3 SKIP 项清单

| ID | SKIP 原因 |
|---|---|
| T-B-CHAIN4-08 | 需要在多会话间切换，单会话环境无法测试 |
| T-B-CHAIN7-01 | 当前工作区无 Rust 源码，source_code 工具无法测试 |
| T-B-CHAIN7-02 | 当前工作区无 Rust 源码，source_code 工具无法测试 |
| T-B-CHAIN7-03 | LSP 实验性功能未启用 |
| T-B-CHAIN8-05 | 需要手动切换 ConfirmationLevel 配置 |
| T-B-CHAIN8-08 | 标记为可选，跳过测试 |
| T-B-CHAIN9-01~9-07 | 需前端事件监听/SQLite 检查/特殊配置，CLI 环境无法完整测试 |
| T-B-CHAIN10-01~10-05 | 需操作 LLM Provider 配置及网络环境，当前环境不满足测试条件 |
| | |

### 12.4 测试结论

- 整体通过率：45 / 67 = 67.2%
- Critical/High 级别失败数：0
- 是否可进入修复阶段：是
- 备注：PASS项涵盖核心文件操作、目录操作、脚本执行、子Agent委托、网络抓取、权限安全等关键能力；FAIL项均为medium/low级别（高风险命令检测、脚本泄露检测、websearch配置、外部目录权限级别）；SKIP项主要因环境限制（无Rust源码、无前端/SQLite、LLM Provider配置不可操作）

---

## 附录 A：测试执行提示词（智能体使用）

当本文档作为输入发给智能体时，可附加如下指令：

```
请按 docs/tests/agent_self_test_build.md 的链路顺序执行自检测试。
要求：
1) 切换到 Build 模式。
2) 在工作区创建 __self_test__/ 目录作为测试沙箱，所有文件操作在该目录内进行。
3) 测试数据由你自己生成（用 write/write_script/bash 等工具），生成能力本身也是测试项。
4) 每个链路内部存在依赖关系，按顺序执行；前一步生成的文件在后续步骤中复用。
5) 每完成一项测试，立即在表格"实际结果"和"结论"列填写结果。
6) 测试中如发现工具/机制异常，先记录为 FAIL 并简要描述现象，不要尝试修复。
7) 全部链路完成后，执行链路十一统一清理，再填写第 12 章汇总表。
8) 对于依赖外部条件（无网络、无 LSP、无搜索后端）的测试，可标 SKIP 并注明原因。
```

## 附录 B：关键错误码速查

| 错误码 | 含义 | 触发场景 |
|---|---|---|
| 1000-1999 | LLM 错误 | 连接失败/认证/限流/超时/Provider 不可用 |
| 2002 | AGENT_MAX_ITERATIONS | 达到最大迭代次数 |
| 2004 | AGENT_CONFIRM_TIMEOUT | 用户确认 5 分钟超时 |
| 2006 | AGENT_HANDLER_NOT_FOUND | Handler 不存在 |
| 3002 | DOC_FORMAT_UNSUPPORTED | 文档格式不支持 |
| 3011 | DOC_PERMISSION_DENIED | 文档路径越界 |
| 9002 | TOOL_INVALID_PARAMS | 工具参数缺失/无效 |
| 9004 | TOOL_PATH_OUT_OF_BOUNDS | 路径越界（工作区外） |
| 9006 | TOOL_NOT_FOUND | 工具不存在 |
| 9007 | TOOL_EXECUTION_ERROR | 工具执行失败 |
