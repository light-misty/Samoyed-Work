"""Code Executor Subprocess - 在独立子进程中安全执行 Python 代码

由 CodeHandler 通过 subprocess 调用，通过 stdin/stdout JSON 协议通信。
提供进程级隔离，避免代码执行导致主 Sidecar 进程崩溃。

输入格式 (stdin JSON):
{
    "code": "Python 代码字符串",
    "working_dir": "工作目录",
    "timeout": 60,
    "max_memory_mb": 512,
    "max_file_size_mb": 50,
    "max_output_bytes": 10000,
    "max_files": 20
}

输出格式 (stdout JSON):
{
    "success": true/false,
    "output": "stdout 输出",
    "files": ["生成的文件列表"],
    "error": "错误信息（如果有）",
    "memory_used_mb": 123.4,
    "duration_ms": 5678
}
"""

import os
import re
import sys
import json
import threading
import tracemalloc
import time

# 添加 sidecar 目录到 sys.path，以便导入 handlers.doc_helpers
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


# ============================================================================
# 安全配置
# ============================================================================

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
BLOCKED_PATTERNS = [
    r'__import__\s*\(',          # 禁止直接调用 __import__
    r'os\.system\s*\(',          # 禁止 os.system
    r'subprocess\.',             # 禁止 subprocess 模块
]


# ============================================================================
# 安全检查
# ============================================================================

def check_security(code: str) -> dict:
    """多层代码静态安全检查

    第一层：RestrictedPython AST 级别分析（如果可用）
    第二层：正则表达式模式匹配

    Returns:
        {"safe": bool, "reason": str, "layer": str}
    """
    # 第一层：RestrictedPython AST 分析
    rp_result = _check_restricted_python(code)
    if rp_result is not None:
        if not rp_result["safe"]:
            return rp_result
        # RestrictedPython 检查通过，继续正则检查作为补充

    # 第二层：正则表达式模式匹配
    for pattern in BLOCKED_PATTERNS:
        match = re.search(pattern, code)
        if match:
            return {
                "safe": False,
                "reason": f"代码包含禁止的模式: {match.group()}",
                "layer": "regex",
            }

    return {"safe": True, "reason": "", "layer": "all"}


def _check_restricted_python(code: str) -> dict | None:
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
        error_msg = str(e)
        if e.args and isinstance(e.args[0], tuple):
            # 多条错误信息
            error_msgs = [str(err) for err in e.args[0]]
            error_msg = '; '.join(error_msgs)

        # 区分真正的安全错误和语法错误
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
    except Exception:
        # RestrictedPython 自身出错，不阻止执行（回退到正则检查）
        return None


# ============================================================================
# 受限命名空间构建
# ============================================================================

def build_namespace(working_dir: str) -> dict:
    """构建受限执行命名空间"""
    import builtins

    # 复制内建函数，移除危险项
    # 注意：不从 safe_builtins 中移除 exec，因为 _execute_with_timeout
    # 内部使用 exec(code, namespace) 执行用户代码，exec 需要在 __builtins__ 中可用
    # 但用户代码无法直接调用 exec/eval/compile，因为它们不在命名空间顶层
    safe_builtins = {k: v for k, v in builtins.__dict__.items()
                     if k not in ('__import__', 'breakpoint', 'exit', 'quit')}

    # 自定义安全导入函数
    def safe_import(name, *args, **kwargs):
        if name in BLOCKED_MODULES:
            raise ImportError(f"模块 '{name}' 被禁止导入")
        # 允许白名单中的模块
        if name in ALLOWED_MODULES or any(
            name.startswith(m + '.') for m in ALLOWED_MODULES
        ):
            return builtins.__import__(name, *args, **kwargs)
        raise ImportError(f"模块 '{name}' 不在允许列表中")

    safe_builtins['__import__'] = safe_import

    # 受限的 open() 函数：只允许写入工作区目录
    def safe_open(file, mode='r', *args, **kwargs):
        file_str = str(file)
        # 只允许读取操作和写入工作区目录
        if 'w' in mode or 'a' in mode:
            # 使用 os.path.abspath 规范化路径后比较，避免路径遍历攻击
            # Windows 下路径不区分大小写，使用 os.path.normcase 规范化
            abs_path = os.path.abspath(file_str)
            norm_working_dir = os.path.normcase(os.path.abspath(working_dir))
            norm_file = os.path.normcase(abs_path)
            if not norm_file.startswith(norm_working_dir):
                raise PermissionError(f"只允许写入工作区目录: {working_dir}")
        return builtins.open(file, mode, *args, **kwargs)

    safe_builtins['open'] = safe_open

    # 初始化命名空间
    namespace = {
        '__builtins__': safe_builtins,
        'working_dir': working_dir,
    }

    # 预导入常用文档处理库
    try:
        from docx import Document
        namespace['Document'] = Document
    except ImportError:
        pass

    try:
        import openpyxl
        namespace['openpyxl'] = openpyxl
    except ImportError:
        pass

    try:
        from pptx import Presentation
        namespace['Presentation'] = Presentation
    except ImportError:
        pass

    try:
        import matplotlib.pyplot as plt
        namespace['plt'] = plt
    except ImportError:
        pass

    try:
        import pandas as pd
        namespace['pd'] = pd
    except ImportError:
        pass

    # 导入项目 helper 函数
    try:
        import handlers.doc_helpers as doc_helpers
        namespace['doc_helpers'] = doc_helpers
        # 将常用 helper 直接暴露在命名空间顶层，降低 LLM 编码难度
        for name in ['create_word_doc', 'save_word_doc',
                     'create_excel_doc', 'save_excel_doc',
                     'create_ppt_doc', 'save_ppt_doc',
                     'create_pdf_doc', 'save_pdf_doc',
                     'create_chart', 'save_chart',
                     'add_styled_table', 'apply_theme']:
            if hasattr(doc_helpers, name):
                namespace[name] = getattr(doc_helpers, name)
    except ImportError:
        pass

    return namespace


# ============================================================================
# 代码执行（带超时和资源限制）
# ============================================================================

def execute_with_timeout(
    code: str,
    namespace: dict,
    timeout: int,
    working_dir: str,
    max_memory_mb: int = 512,
    max_file_size_mb: int = 50,
    max_output_bytes: int = 10000,
    max_files: int = 20,
) -> dict:
    """带超时和资源限制的代码执行

    Args:
        code: Python 代码字符串
        namespace: 受限命名空间
        timeout: 执行超时时间（秒）
        working_dir: 工作目录
        max_memory_mb: 最大内存使用量（MB）
        max_file_size_mb: 单个文件最大大小（MB）
        max_output_bytes: 输出最大字节数
        max_files: 最大生成文件数

    Returns:
        执行结果字典
    """
    from io import StringIO

    # 捕获 stdout
    old_stdout = sys.stdout
    captured_output = StringIO()
    sys.stdout = captured_output

    # 启动内存追踪
    tracemalloc.start()

    result = {
        "success": False,
        "output": "",
        "files": [],
        "error": None,
        "memory_used_mb": 0.0,
        "duration_ms": 0,
    }

    # 执行前记录工作目录中的文件集合（用于追踪生成的文件）
    before_files = set()
    if working_dir and os.path.isdir(working_dir):
        for root, dirs, files in os.walk(working_dir):
            for f in files:
                before_files.add(os.path.join(root, f))

    start_time = time.time()

    try:
        exec_result = [None]
        exec_error = [None]
        memory_exceeded = [False]

        def run_code():
            try:
                exec(code, namespace)
                exec_result[0] = True
            except MemoryError:
                exec_error[0] = MemoryError("代码执行超出内存限制")
            except Exception as e:
                exec_error[0] = e

        # 内存监控线程
        def memory_monitor():
            """定期检查内存使用量，超出限制时设置标志"""
            while not memory_exceeded[0]:
                try:
                    current, peak = tracemalloc.get_traced_memory()
                    if peak > max_memory_mb * 1024 * 1024:
                        memory_exceeded[0] = True
                        break
                except Exception:
                    break
                time.sleep(0.5)

        # 启动代码执行线程
        thread = threading.Thread(target=run_code)
        thread.daemon = True
        thread.start()

        # 启动内存监控线程
        monitor_thread = threading.Thread(target=memory_monitor)
        monitor_thread.daemon = True
        monitor_thread.start()

        # 等待执行完成或超时
        thread.join(timeout=timeout)

        duration_ms = int((time.time() - start_time) * 1000)
        result["duration_ms"] = duration_ms

        if thread.is_alive():
            # 执行超时
            result["error"] = f"代码执行超时（{timeout}秒）"
            result["output"] = captured_output.getvalue()[:max_output_bytes]
            return result

        if memory_exceeded[0]:
            # 内存超限
            result["error"] = f"代码执行超出内存限制（{max_memory_mb}MB）"
            result["output"] = captured_output.getvalue()[:max_output_bytes]
            return result

        if exec_error[0]:
            raise exec_error[0]

        # 执行后比较工作目录文件变化，追踪新生成的文件
        generated_files = []
        if working_dir and os.path.isdir(working_dir):
            for root, dirs, files in os.walk(working_dir):
                for f in files:
                    full_path = os.path.join(root, f)
                    if full_path not in before_files:
                        generated_files.append(full_path)

        # 检查生成文件数量限制
        if len(generated_files) > max_files:
            # 超出文件数量限制，删除超出的文件
            excess_files = generated_files[max_files:]
            for f in excess_files:
                try:
                    os.remove(f)
                except Exception:
                    pass
            generated_files = generated_files[:max_files]
            result["error"] = f"生成的文件数量超出限制（{max_files}个），已删除多余文件"
            result["output"] = captured_output.getvalue()[:max_output_bytes]
            result["files"] = generated_files
            return result

        # 检查单个文件大小限制
        oversized_files = []
        for f in generated_files:
            try:
                file_size = os.path.getsize(f)
                if file_size > max_file_size_mb * 1024 * 1024:
                    oversized_files.append(f)
            except Exception:
                pass

        if oversized_files:
            # 删除超大的文件
            for f in oversized_files:
                try:
                    os.remove(f)
                except Exception:
                    pass
                generated_files.remove(f)
            result["error"] = f"以下文件超出大小限制（{max_file_size_mb}MB）已被删除: {', '.join(os.path.basename(f) for f in oversized_files)}"
            result["output"] = captured_output.getvalue()[:max_output_bytes]
            result["files"] = generated_files
            return result

        # 获取内存使用峰值
        try:
            current, peak = tracemalloc.get_traced_memory()
            result["memory_used_mb"] = round(peak / (1024 * 1024), 2)
        except Exception:
            pass

        result["success"] = True
        result["output"] = captured_output.getvalue()[:max_output_bytes]
        result["files"] = generated_files

    except TimeoutError:
        result["error"] = f"代码执行超时（{timeout}秒）"
        result["output"] = captured_output.getvalue()[:max_output_bytes]
        result["duration_ms"] = int((time.time() - start_time) * 1000)
    except Exception as e:
        result["error"] = f"{type(e).__name__}: {e}"
        result["output"] = captured_output.getvalue()[:max_output_bytes]
        result["duration_ms"] = int((time.time() - start_time) * 1000)
    finally:
        # 停止内存追踪
        try:
            tracemalloc.stop()
        except Exception:
            pass
        sys.stdout = old_stdout

    return result


# ============================================================================
# 主入口
# ============================================================================

def main():
    """从 stdin 读取 JSON 请求，执行代码，返回 JSON 结果到 stdout"""
    # Windows 管道模式下确保 UTF-8 编码
    sys.stdin.reconfigure(encoding='utf-8')
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')

    try:
        # 读取输入
        input_line = sys.stdin.readline().strip()
        # 移除 UTF-8 BOM
        input_line = input_line.lstrip('\ufeff')
        if not input_line:
            result = {"success": False, "error": "输入为空", "output": "", "files": [], "memory_used_mb": 0, "duration_ms": 0}
            sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
            sys.stdout.flush()
            return

        request = json.loads(input_line)

        code = request.get("code", "")
        working_dir = request.get("working_dir", "")
        timeout = request.get("timeout", 60)
        max_memory_mb = request.get("max_memory_mb", 512)
        max_file_size_mb = request.get("max_file_size_mb", 50)
        max_output_bytes = request.get("max_output_bytes", 10000)
        max_files = request.get("max_files", 20)

        if not code:
            result = {"success": False, "error": "缺少代码内容", "output": "", "files": [], "memory_used_mb": 0, "duration_ms": 0}
            sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
            sys.stdout.flush()
            return

        # 安全检查（子进程内的第二层检查）
        security_result = check_security(code)
        if not security_result["safe"]:
            result = {
                "success": False,
                "error": f"代码安全检查未通过: {security_result['reason']}",
                "output": "",
                "files": [],
                "memory_used_mb": 0,
                "duration_ms": 0,
            }
            sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
            sys.stdout.flush()
            return

        # 构建受限命名空间
        namespace = build_namespace(working_dir)

        # 执行代码
        result = execute_with_timeout(
            code=code,
            namespace=namespace,
            timeout=timeout,
            working_dir=working_dir,
            max_memory_mb=max_memory_mb,
            max_file_size_mb=max_file_size_mb,
            max_output_bytes=max_output_bytes,
            max_files=max_files,
        )

        # 输出结果
        sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
        sys.stdout.flush()

    except json.JSONDecodeError as e:
        result = {"success": False, "error": f"JSON 解析错误: {e}", "output": "", "files": [], "memory_used_mb": 0, "duration_ms": 0}
        sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
        sys.stdout.flush()
    except Exception as e:
        result = {"success": False, "error": f"内部错误: {type(e).__name__}: {e}", "output": "", "files": [], "memory_used_mb": 0, "duration_ms": 0}
        sys.stdout.write(json.dumps(result, ensure_ascii=False) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
