# Samoyed Work 智能体自检测试文档（Document 模式）

> **用途**：将本文档原样发送给智能体（Document 模式），由智能体按"链路"顺序一条龙执行测试。
> **核心思想**：测试样例文档由智能体自己通过脚本生成（顺便测试脚本生成与执行能力），生成后串联 Handler 读取/转换/修改/验证等测试，最后统一测删除。每个链路内部存在依赖关系，按顺序执行。
> **适用模式**：**Document 模式**（4 个文档 Handler 可用；Build 模式工具测试见 `agent_self_test_build.md`）
> **填写规范**：在"实际结果"列填写观察到的关键现象（不要粘贴大段日志）；"结论"列填写 `PASS` / `FAIL` / `SKIP`（SKIP 需注明原因）。

---

## 0. 测试前置准备

| 项 | 要求 |
|---|---|
| 工作区 | 当前工作区可写，由智能体创建 `__self_test__/` 子目录作为测试沙箱，链路全部结束后清理 |
| LLM Provider | 至少配置 1 个可用 Provider（OpenAI/Anthropic/Gemini 任一） |
| Agent 模式 | **Document 模式**（确保 4 个文档 Handler 出现在工具列表中） |
| Python Sidecar | 已安装 python-docx/openpyxl/python-pptx/reportlab/PyMuPDF 等依赖（`pip install -r sidecar/requirements.txt`） |
| 网络 | 默认联网；链路七需联网（用户偏好注入不强制，但跨会话验证需重启会话） |
| 日志 | `log/samoyed_work.log`（Rust）和 `src-tauri/target/debug/log/sidecar.log`（Sidecar） |

**统一清理动作**（全部链路完成后执行）：删除 `__self_test__/` 目录及其所有内容。

---

## 1. 链路一：docx 生成与处理（脚本生成→Handler读取→转换→验证）

> 本链路由智能体通过 `write_script` + `bash` 调用 python-docx 生成样例文档，再用 docx Handler 读取/转换。

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-D-CHAIN1-01 | 1.1 准备目录 | `mkdir` | 创建 `__self_test__/` 目录（recursive=true） | 目录创建成功 | | |
| T-D-CHAIN1-02 | 1.2 生成 docx | `write_script` + `bash` | 用 python-docx 生成 `__self_test__/sample.docx`，含若干段落、1 个表格、1 个标题 | 文件创建成功；`file_info` 显示 `file_type=word`、`extension=docx` | | |
| T-D-CHAIN1-03 | 1.3 docx 读取 | `docx` | 调用 `docx_handler` `action=read`，含 `include_tables_detailed=true` | 返回段落和表格结构化数据；表格内容正确 | | |
| T-D-CHAIN1-04 | 1.4 docx 转换 | `docx` | `action=convert, target_format=md` | 在同目录生成 `.md` 文件；内容含标题和段落 | | |
| T-D-CHAIN1-05 | 1.5 docx 验证 | `validator` | 对 `sample.docx` 调用 `validator_handler`（path + 可选 doc_type） | 返回 `warnings` 列表和 `stats` 统计；显式 `doc_type="docx"` 与从扩展名推断一致 | | |

---

## 2. 链路二：xlsx 生成与处理（脚本生成→Handler读取→转换）

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-D-CHAIN2-01 | 2.1 生成 xlsx | `write_script` + `bash` | 用 openpyxl 生成 `__self_test__/sample.xlsx`，含 2 个 sheet 和公式（如 SUM） | 文件创建成功；`file_info` 显示 `file_type=excel`、`extension=xlsx` | | |
| T-D-CHAIN2-02 | 2.2 xlsx 读取 | `xlsx` | `action=read, include_formulas=true` | 返回 sheet 数据和公式；2 个 sheet 均返回 | | |
| T-D-CHAIN2-03 | 2.3 xlsx 转换 | `xlsx` | `action=convert, target_format=csv` | 生成 csv 文件；内容含表格数据 | | |

---

## 3. 链路三：pptx 生成与处理（脚本生成→Handler读取）

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-D-CHAIN3-01 | 3.1 生成 pptx | `write_script` + `bash` | 用 python-pptx 生成 `__self_test__/sample.pptx`，含 2 张幻灯片和备注 | 文件创建成功；`file_info` 显示 `file_type=powerpoint`、`extension=pptx` | | |
| T-D-CHAIN3-02 | 3.2 pptx 读取 | `pptx` | `action=read, include_notes=true` | 返回幻灯片内容和备注；2 张幻灯片均返回 | | |

---

## 4. 链路四：pdf 生成与处理（脚本生成→Handler读取→修改子操作）

> PDF Handler 支持 17 种 modify 子操作，本链路测试 read + 至少 2 种 modify 子操作。

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-D-CHAIN4-01 | 4.1 生成 pdf | `write_script` + `bash` | 用 reportlab 生成 `__self_test__/sample.pdf`，含 3 页文本 | 文件创建成功；`file_info` 显示 `file_type=pdf`、`extension=pdf` | | |
| T-D-CHAIN4-02 | 4.2 pdf 读取 | `pdf` | `action=read, pages="1-2", include_layout=true` | 返回指定页内容，`pages="1-2"` 过滤生效（只返回前 2 页） | | |
| T-D-CHAIN4-03 | 4.3 pdf 修改 - split | `pdf` | `action=modify, operation=split, pages="1"`，输出到 `__self_test__/split/` | 在 `split/` 目录生成只含第 1 页的新 PDF | | |
| T-D-CHAIN4-04 | 4.4 pdf 修改 - 其他子操作 | `pdf` | 任选 1 个子操作测试（建议 `add_text_watermark` 或 `encrypt` 或 `merge`） | 子操作执行成功，生成对应产物 | | |

---

## 5. 链路五：validator 综合验证（多文档类型验证）

> 对前面链路生成的多种文档类型进行验证，测试 validator 的多类型支持。

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-D-CHAIN5-01 | 5.1 验证 xlsx | `validator` | 对 `sample.xlsx` 调用 `validator_handler`（不传 doc_type，测试从扩展名推断） | 返回 `warnings` 和 `stats`；`doc_type` 自动推断为 `xlsx` | | |
| T-D-CHAIN5-02 | 5.2 验证 pptx | `validator` | 对 `sample.pptx` 调用 `validator_handler`（显式传 `doc_type="pptx"`） | 返回 `warnings` 和 `stats`；显式 doc_type 与推断一致 | | |

---

## 6. 链路六：子 Agent 文档委托（Document模式下子Agent可调用文档Handler）

> 验证 Document 模式下子 Agent 继承父 Agent 的文档处理能力。

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-D-CHAIN6-01 | 6.1 子 Agent 读 docx | `task` | 委托子任务：`"用 docx_handler 读取 __self_test__/sample.docx 并返回段落数量"` | 子 Agent 成功调用 docx Handler 并返回结果；前端收到 `agent:sub_agent_status` 事件 | | |
| T-D-CHAIN6-02 | 6.2 子 Agent 生成文档 | `task` | 委托子任务：`"用 write_script + bash 生成 __self_test__/subtask_doc.docx，含 1 个段落"` | 子 Agent 成功生成文档；文件存在 | | |

---

## 7. 链路七：用户偏好注入（多次转换→检查偏好记录→新会话验证注入）

> 验证情景记忆系统的用户偏好提取与注入机制（Document 模式特有）。

| ID | 步骤 | 机制 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-D-CHAIN7-01 | 7.1 触发偏好提取 | `docx` convert | 多次调用 `docx_handler` `action=convert, target_format=md`（至少 3 次，可对不同文档） | 每次转换成功；`user_preferences` 表存在 `preferred_document_format` 或 `target_format` 偏好记录，`confidence ≥ 0.7` | | |
| T-D-CHAIN7-02 | 7.2 新会话偏好注入 | 情景记忆 | 新建会话，检查 system_prompt 末尾 | 新会话 system_prompt 含 `<user_preferences>` 块，记录了文档格式偏好 | | |

---

## 8. 链路八：统一清理

| ID | 步骤 | 工具 | 操作 | 预期结果 | 实际结果 | 结论 |
|---|---|---|---|---|---|---|
| T-D-CLEAN-01 | 8.1 清理 | `remove_dir` | 删除 `__self_test__/` 目录 | **应弹出确认**；确认后递归删除成功；工作区根目录保留 | | |

---

## 9. 测试结果汇总

### 9.1 统计

| 链路 | 总数 | PASS | FAIL | SKIP |
|---|---|---|---|---|
| 1. docx 生成与处理 | 5 | | | |
| 2. xlsx 生成与处理 | 3 | | | |
| 3. pptx 生成与处理 | 2 | | | |
| 4. pdf 生成与处理 | 4 | | | |
| 5. validator 综合验证 | 2 | | | |
| 6. 子 Agent 文档委托 | 2 | | | |
| 7. 用户偏好注入 | 2 | | | |
| 8. 统一清理 | 1 | | | |
| **合计** | **21** | | | |

### 9.2 失败项清单

> 将所有 FAIL 项在此列出，按严重程度排序（critical > high > medium > low），并附简要原因分析，供后续修复参考。

| ID | 严重程度 | 失败现象 | 可能原因 | 建议修复方向 |
|---|---|---|---|---|
| | | | | |

### 9.3 SKIP 项清单

| ID | SKIP 原因 |
|---|---|
| | |

### 9.4 测试结论

- 整体通过率：____ / 21 = ____%
- Critical/High 级别失败数：____
- 是否可进入修复阶段：是 / 否
- 备注：

---

## 附录 A：测试执行提示词（智能体使用）

当本文档作为输入发给智能体时，可附加如下指令：

```
请按 docs/tests/agent_self_test_document.md 的链路顺序执行自检测试。
要求：
1) 切换到 Document 模式（确保 docx/xlsx/pptx/pdf Handler 出现在工具列表中）。
2) 在工作区创建 __self_test__/ 目录作为测试沙箱，所有文件操作在该目录内进行。
3) 测试样例文档由你自己通过 write_script + bash 调用 python-docx/openpyxl/python-pptx/reportlab 生成，生成能力本身也是测试项。
4) 每个链路内部存在依赖关系，按顺序执行；前一步生成的文档在后续步骤中复用。
5) 每完成一项测试，立即在表格"实际结果"和"结论"列填写结果。
6) 测试中如发现工具/机制异常，先记录为 FAIL 并简要描述现象，不要尝试修复。
7) 全部链路完成后，执行链路八统一清理，再填写第 9 章汇总表。
8) 对于依赖外部条件（无网络、无 LLM、Python 依赖缺失）的测试，可标 SKIP 并注明原因。
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

## 附录 C：PDF modify 17 种子操作参考

> 链路四步骤 4.4 可从中任选 1 个测试（建议 `add_text_watermark` 或 `encrypt`）。

| 类别 | 子操作 |
|---|---|
| 页面操作 | `rotate_pages`、`delete_pages`、`extract_pages`、`reorder_pages` |
| 合并拆分 | `merge`、`split` |
| 水印 | `add_text_watermark`、`add_image_watermark` |
| 页眉页脚 | `add_header_footer` |
| 加密解密 | `encrypt`、`decrypt` |
| 元数据 | `set_metadata` |
| 书签目录 | `add_bookmarks`、`set_toc` |
| 注释 | `add_annotation` |
| 表单填写 | `fill_form` |
| 压缩 | `compress` |
