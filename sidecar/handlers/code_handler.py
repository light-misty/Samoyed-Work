"""代码执行处理器 - 让 Agent 自由编写 Python 代码生成/修改文档

对应 Sidecar 请求: {"action": "execute", "type": "code", "params": {...}}

安全架构（Phase 4 加固后）:
- L1: RestrictedPython AST 级别静态分析（主进程预检）
- L2: 正则表达式模式匹配（主进程预检）
- L3: 子进程沙箱隔离（独立进程执行，避免主 Sidecar 崩溃）
- L4: 受限命名空间（子进程内，白名单导入 + safe_open）
- L5: 执行超时（子进程内，线程超时 + 进程超时双保险）
- L6: 资源限制（子进程内，内存追踪 + 文件大小/数量限制）
- L7: 用户确认（Rust 端，HIGH_RISK_SKILLS 常量）
- L8: 审计日志（主进程，记录所有代码执行历史）
"""

import hashlib
import json
import logging
import os
import re
import subprocess
import sys
import time

logger = logging.getLogger(__name__)


class CodeHandler:
    """代码执行处理器 - 让 Agent 自由编写 Python 代码生成/修改文档

    Phase 4 安全加固:
    1. RestrictedPython AST 级别静态分析
    2. 子进程沙箱隔离
    3. 资源限制（内存/文件大小/文件数量）
    4. 审计日志
    """

    # 允许的 Python 模块白名单
    ALLOWED_MODULES = {
        # 文档处理库
        "docx", "openpyxl", "pptx", "reportlab", "fpdf",
        # 数据处理库
        "pandas", "numpy", "csv", "json", "math", "statistics",
        # 图表库
        "matplotlib", "plotly",
        # 图像处理库
        "PIL", "pillow",
        # 日期时间
        "datetime", "dateutil", "time",
        # 正则表达式
        "re",
        # 路径和系统操作（受限，safe_open 控制文件写入）
        "os", "os.path", "pathlib", "sys",
        # 类型相关
        "typing", "collections", "copy",
        # 编码
        "base64", "hashlib",
        # 随机数
        "random",
        # 项目内部 helper
        "doc_helpers",
    }

    # 禁止的模块黑名单（即使白名单中未列出也做二次拦截）
    BLOCKED_MODULES = {
        "subprocess", "socket", "http", "urllib",
        "shutil", "signal", "ctypes", "multiprocessing",
        "webbrowser", "telnetlib", "ftplib", "smtplib",
        "xmlrpc", "pickle", "shelve", "marshal",
    }

    # 禁止的代码模式（正则表达式）
    # 注意：不禁止 exec/eval/compile，因为 _execute_with_timeout 内部
    # 使用 exec() 执行用户代码，静态检查只拦截用户代码中的嵌套调用
    # 注意：不拦截 `import os`，因为 os.path 是白名单模块的子模块，
    # Python 中 `import os.path` 实际也会导入 os。os 模块的导入控制
    # 由 safe_import 统一处理（只允许访问 os.path 等子模块，禁止 os.system 等）
    BLOCKED_PATTERNS = [
        r'__import__\s*\(',          # 禁止直接调用 __import__
        r'os\.system\s*\(',          # 禁止 os.system
        r'subprocess\.',             # 禁止 subprocess 模块
    ]

    # 资源限制默认值
    DEFAULT_MAX_MEMORY_MB = 512       # 最大内存使用量（MB）
    DEFAULT_MAX_FILE_SIZE_MB = 50     # 单个文件最大大小（MB）
    DEFAULT_MAX_OUTPUT_BYTES = 10000  # 输出最大字节数
    DEFAULT_MAX_FILES = 20            # 最大生成文件数

    # 子进程超时缓冲时间（秒），比代码执行超时多出的时间
    # 用于让子进程有时间返回结果，而非被主进程强制终止
    SUBPROCESS_TIMEOUT_BUFFER = 10

    def execute(self, params: dict) -> dict:
        """执行 Python 代码生成/修改文档

        对应 Sidecar 请求: {"action": "execute", "type": "code", "params": {...}}

        params:
            code: Python 代码字符串
            working_dir: 工作目录（文件输出目录）
            timeout: 执行超时时间（秒），默认 60
        """
        code = params.get("code", "")
        working_dir = params.get("working_dir", "")
        timeout = params.get("timeout", 60)

        if not code:
            return {"error": "缺少代码内容"}

        # 记录执行开始时间
        start_time = time.time()

        # ===== L1 + L2: 主进程安全预检 =====
        # 在子进程启动前进行安全检查，避免为危险代码启动子进程的开销
        security_check = self._check_security(code)
        if not security_check["safe"]:
            # 记录审计日志（安全检查失败）
            self._write_audit_log(
                code=code, working_dir=working_dir, timeout=timeout,
                result="security_blocked", error=security_check["reason"],
                duration_ms=int((time.time() - start_time) * 1000),
                security_check=security_check,
            )
            return {"error": f"代码安全检查未通过: {security_check['reason']}"}

        # ===== L3: 子进程沙箱执行 =====
        try:
            result = self._execute_in_subprocess(
                code=code,
                working_dir=working_dir,
                timeout=timeout,
            )
        except Exception as e:
            error_msg = f"子进程执行失败: {type(e).__name__}: {e}"
            # 记录审计日志（子进程异常）
            self._write_audit_log(
                code=code, working_dir=working_dir, timeout=timeout,
                result="subprocess_error", error=error_msg,
                duration_ms=int((time.time() - start_time) * 1000),
                security_check=security_check,
            )
            return {"error": error_msg}

        # ===== L8: 审计日志 =====
        duration_ms = int((time.time() - start_time) * 1000)
        self._write_audit_log(
            code=code, working_dir=working_dir, timeout=timeout,
            result="success" if result.get("success") else "execution_error",
            error=result.get("error"),
            files=result.get("files", []),
            memory_used_mb=result.get("memory_used_mb", 0),
            duration_ms=duration_ms,
            executor_duration_ms=result.get("duration_ms", 0),
            security_check=security_check,
        )

        return result

    def _check_security(self, code: str) -> dict:
        """多层代码静态安全检查

        第一层：RestrictedPython AST 级别分析（如果可用）
        第二层：正则表达式模式匹配

        Returns:
            {"safe": bool, "reason": str, "layer": str}
        """
        # 第一层：RestrictedPython AST 分析
        rp_result = self._check_restricted_python(code)
        if rp_result is not None:
            if not rp_result["safe"]:
                return rp_result
            # RestrictedPython 检查通过，继续正则检查作为补充

        # 第二层：正则表达式模式匹配
        for pattern in self.BLOCKED_PATTERNS:
            match = re.search(pattern, code)
            if match:
                return {
                    "safe": False,
                    "reason": f"代码包含禁止的模式: {match.group()}",
                    "layer": "regex",
                }

        return {"safe": True, "reason": "", "layer": "all"}

    def _check_restricted_python(self, code: str) -> dict | None:
        """使用 RestrictedPython 进行 AST 级别安全检查

        RestrictedPython 8.x API:
        - 成功时返回 code 对象
        - 有安全错误时抛出 SyntaxError（包含错误元组）
        - 有警告时发出 SyntaxWarning

        Returns:
            None 如果 RestrictedPython 不可用
            {"safe": bool, "reason": str, "layer": "restricted_python"} 检查结果
        """
        try:
            from RestrictedPython import compile_restricted
        except ImportError:
            # RestrictedPython 未安装，跳过此层检查
            return None

        try:
            # 尝试编译受限代码
            # RestrictedPython 8.x: 成功返回 code 对象，失败抛出 SyntaxError
            compile_restricted(code, "<code_interpreter>", "exec")
            return {"safe": True, "reason": "", "layer": "restricted_python"}

        except SyntaxError as e:
            # RestrictedPython 安全检查错误
            # e.args[0] 通常是错误元组，如 ('Line 1: "__import__" is an invalid attribute name...',)
            error_msg = str(e)
            if e.args and isinstance(e.args[0], tuple):
                # 多条错误信息
                error_msgs = [str(err) for err in e.args[0]]
                error_msg = '; '.join(error_msgs)

            # 区分真正的安全错误和语法错误
            # 安全错误通常包含 "is an invalid attribute name" 或 "is not allowed"
            security_keywords = [
                "invalid attribute name",
                "is not allowed",
                "is an invalid variable name",
                "is not a valid",
                "forbidden",
            ]
            is_security_error = any(kw in error_msg for kw in security_keywords)

            if is_security_error:
                return {
                    "safe": False,
                    "reason": f"RestrictedPython 安全检查未通过: {error_msg}",
                    "layer": "restricted_python",
                }
            else:
                # 纯语法错误，也阻止执行
                return {
                    "safe": False,
                    "reason": f"代码语法错误: {error_msg}",
                    "layer": "restricted_python",
                }
        except Exception as e:
            # RestrictedPython 自身出错，不阻止执行（回退到正则检查）
            logger.warning("RestrictedPython 检查异常: %s: %s", type(e).__name__, e)
            return None

    def _execute_in_subprocess(
        self,
        code: str,
        working_dir: str,
        timeout: int,
    ) -> dict:
        """在独立子进程中执行代码（沙箱隔离）

        通过 subprocess 调用 code_executor.py，实现进程级隔离。
        如果代码执行导致子进程崩溃，主 Sidecar 进程不受影响。

        Args:
            code: Python 代码字符串
            working_dir: 工作目录
            timeout: 执行超时时间（秒）

        Returns:
            执行结果字典
        """
        # 定位 code_executor.py 脚本路径（与当前文件同目录的上一级）
        sidecar_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
        executor_script = os.path.join(sidecar_dir, "code_executor.py")

        if not os.path.exists(executor_script):
            logger.error("代码执行器脚本不存在: %s", executor_script)
            return {"error": f"代码执行器脚本不存在: {executor_script}"}

        # 构建子进程输入
        executor_input = json.dumps({
            "code": code,
            "working_dir": working_dir,
            "timeout": timeout,
            "max_memory_mb": self.DEFAULT_MAX_MEMORY_MB,
            "max_file_size_mb": self.DEFAULT_MAX_FILE_SIZE_MB,
            "max_output_bytes": self.DEFAULT_MAX_OUTPUT_BYTES,
            "max_files": self.DEFAULT_MAX_FILES,
        }, ensure_ascii=False)

        # 获取 Python 解释器路径（使用当前 Sidecar 相同的解释器）
        python_path = sys.executable

        # 子进程总超时 = 代码执行超时 + 缓冲时间
        subprocess_timeout = timeout + self.SUBPROCESS_TIMEOUT_BUFFER

        try:
            logger.info(
                "启动代码执行子进程: timeout=%ds, working_dir=%s",
                timeout, working_dir
            )

            # 启动子进程
            proc = subprocess.run(
                [python_path, executor_script],
                input=executor_input,
                capture_output=True,
                text=True,
                encoding='utf-8',
                timeout=subprocess_timeout,
                # Windows 平台：不弹出命令行窗口
                creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == 'win32' else 0,
            )

            # 检查子进程退出码
            if proc.returncode != 0:
                stderr_output = proc.stderr[:2000] if proc.stderr else ""
                logger.error(
                    "代码执行子进程异常退出: returncode=%d, stderr=%s",
                    proc.returncode, stderr_output
                )
                return {
                    "error": f"代码执行子进程异常退出 (code={proc.returncode}): {stderr_output}",
                }

            # 解析子进程输出
            stdout = proc.stdout.strip()
            if not stdout:
                return {"error": "代码执行子进程未返回结果"}

            # 去除 UTF-8 BOM
            stdout = stdout.lstrip('\ufeff')

            try:
                result = json.loads(stdout)
            except json.JSONDecodeError as e:
                logger.error("代码执行子进程输出解析失败: %s, 原始输出: %s", e, stdout[:500])
                return {"error": f"代码执行结果解析失败: {e}"}

            logger.info(
                "代码执行子进程完成: success=%s, duration_ms=%d, memory_used_mb=%.1f",
                result.get("success"), result.get("duration_ms", 0),
                result.get("memory_used_mb", 0),
            )

            return result

        except subprocess.TimeoutExpired:
            logger.error("代码执行子进程超时（%d秒）", subprocess_timeout)
            return {"error": f"代码执行超时（{timeout}秒）"}
        except Exception as e:
            logger.error("代码执行子进程异常: %s: %s", type(e).__name__, e)
            return {"error": f"代码执行子进程异常: {type(e).__name__}: {e}"}

    def _write_audit_log(
        self,
        code: str,
        working_dir: str,
        timeout: int,
        result: str,
        error: str | None = None,
        files: list | None = None,
        memory_used_mb: float = 0.0,
        duration_ms: int = 0,
        executor_duration_ms: int = 0,
        security_check: dict | None = None,
    ):
        """写入代码执行审计日志

        审计日志记录所有代码执行历史，包括成功、失败和安全检查拦截。
        日志格式为 JSON Lines（每行一条 JSON 记录），便于后续分析和回溯。

        Args:
            code: 执行的代码
            working_dir: 工作目录
            timeout: 超时设置
            result: 执行结果类型 (success/execution_error/security_blocked/subprocess_error)
            error: 错误信息（如果有）
            files: 生成的文件列表
            memory_used_mb: 内存使用量（MB）
            duration_ms: 总耗时（毫秒）
            executor_duration_ms: 子进程执行耗时（毫秒）
            security_check: 安全检查结果
        """
        try:
            # 计算代码哈希（用于追踪相同代码的多次执行）
            code_hash = hashlib.sha256(code.encode('utf-8')).hexdigest()[:16]

            # 代码预览（截取前 200 字符，避免日志过大）
            code_preview = code[:200].replace('\n', '\\n') if code else ""

            # 构建审计记录
            audit_record = {
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
                "event": "code_execute",
                "code_hash": code_hash,
                "code_preview": code_preview,
                "code_length": len(code),
                "working_dir": working_dir,
                "timeout": timeout,
                "result": result,
                "error": error[:500] if error else None,
                "files": [os.path.basename(f) for f in (files or [])],
                "file_count": len(files) if files else 0,
                "memory_used_mb": memory_used_mb,
                "duration_ms": duration_ms,
                "executor_duration_ms": executor_duration_ms,
                "security_check_layer": security_check.get("layer", "") if security_check else "",
            }

            # 确定审计日志文件路径
            project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            log_dir = os.path.join(project_root, "log")
            os.makedirs(log_dir, exist_ok=True)
            audit_log_path = os.path.join(log_dir, "code_audit.log")

            # 追加写入审计日志（JSON Lines 格式）
            with open(audit_log_path, 'a', encoding='utf-8') as f:
                f.write(json.dumps(audit_record, ensure_ascii=False) + "\n")

            logger.debug("审计日志已写入: result=%s, code_hash=%s", result, code_hash)

        except Exception as e:
            # 审计日志写入失败不应影响代码执行结果的返回
            logger.warning("审计日志写入失败: %s: %s", type(e).__name__, e)
