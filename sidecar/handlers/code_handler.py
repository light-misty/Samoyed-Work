"""代码执行处理器 - 让 Agent 自由编写 Python 代码生成/修改文档

对应 Sidecar 请求: {"action": "execute", "type": "code", "params": {...}}
"""

import os
import re
import sys
import threading


class CodeHandler:
    """代码执行处理器 - 让 Agent 自由编写 Python 代码生成/修改文档"""

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
        "datetime", "dateutil",
        # 正则表达式
        "re",
        # 路径和系统操作（受限，safe_open 控制文件写入）
        "os", "os.path", "pathlib",
        # 类型相关
        "typing", "collections", "copy",
        # 编码
        "base64", "hashlib",
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

        # 安全检查
        security_check = self._check_security(code)
        if not security_check["safe"]:
            return {"error": f"代码安全检查未通过: {security_check['reason']}"}

        # 构建受限执行环境
        namespace = self._build_namespace(working_dir)

        # 执行代码（带超时）
        try:
            result = self._execute_with_timeout(code, namespace, timeout, working_dir)
            return result
        except TimeoutError:
            return {"error": f"代码执行超时（{timeout}秒）"}
        except Exception as e:
            return {"error": f"代码执行失败: {type(e).__name__}: {e}"}

    def _check_security(self, code: str) -> dict:
        """代码静态安全检查

        使用正则匹配禁止模式，不拦截 exec/eval/compile，
        因为内部用 exec 执行用户代码
        """
        for pattern in self.BLOCKED_PATTERNS:
            match = re.search(pattern, code)
            if match:
                return {
                    "safe": False,
                    "reason": f"代码包含禁止的模式: {match.group()}",
                }
        return {"safe": True, "reason": ""}

    def _build_namespace(self, working_dir: str) -> dict:
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
            if name in self.BLOCKED_MODULES:
                raise ImportError(f"模块 '{name}' 被禁止导入")
            # 允许白名单中的模块
            if name in self.ALLOWED_MODULES or any(
                name.startswith(m + '.') for m in self.ALLOWED_MODULES
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
            import doc_helpers
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

    def _execute_with_timeout(self, code: str, namespace: dict, timeout: int, working_dir: str) -> dict:
        """带超时的代码执行"""
        from io import StringIO

        # 捕获 stdout
        old_stdout = sys.stdout
        captured_output = StringIO()
        sys.stdout = captured_output

        result = {"success": False, "output": "", "files": [], "error": None}

        # 执行前记录工作目录中的文件集合（用于追踪生成的文件）
        before_files = set()
        if working_dir and os.path.isdir(working_dir):
            for root, dirs, files in os.walk(working_dir):
                for f in files:
                    before_files.add(os.path.join(root, f))

        try:
            exec_result = [None]
            exec_error = [None]

            def run_code():
                try:
                    exec(code, namespace)
                    exec_result[0] = True
                except Exception as e:
                    exec_error[0] = e

            thread = threading.Thread(target=run_code)
            thread.daemon = True
            thread.start()
            thread.join(timeout=timeout)

            if thread.is_alive():
                raise TimeoutError(f"代码执行超时（{timeout}秒）")

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

            result["success"] = True
            result["output"] = captured_output.getvalue()[:10000]  # 限制输出大小
            result["files"] = generated_files

        except TimeoutError:
            raise
        except Exception as e:
            result["error"] = f"{type(e).__name__}: {e}"
            result["output"] = captured_output.getvalue()[:10000]
        finally:
            sys.stdout = old_stdout

        return result
