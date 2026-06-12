"""DocAgent Python Sidecar
文档处理引擎，通过 stdin/stdout JSON 协议与 Rust 后端通信
支持 Word、Excel、PPT、PDF、Markdown 等文档的生成、读取、修改、转换
"""

import sys
import os
import json
import logging
import traceback
from typing import Any

from handlers.word_handler import WordHandler
from handlers.excel_handler import ExcelHandler
from handlers.ppt_handler import PptHandler
from handlers.pdf_handler import PdfHandler
from handlers.markdown_handler import MarkdownHandler
from handlers.code_handler import CodeHandler
from handlers.validator import DocumentValidator

logger = logging.getLogger(__name__)


def setup_logging():
    project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    log_dir = os.path.join(project_root, "log")
    os.makedirs(log_dir, exist_ok=True)
    log_file = os.path.join(log_dir, "sidecar.log")

    formatter = logging.Formatter(
        fmt='%(asctime)s.%(msecs)03d [%(levelname)-5s] %(name)s - %(message)s',
        datefmt='%Y-%m-%d %H:%M:%S',
    )

    # mode='w' 覆盖写入，每次启动时清空上一次的日志
    file_handler = logging.FileHandler(log_file, mode='w', encoding='utf-8')
    file_handler.setLevel(logging.DEBUG)
    file_handler.setFormatter(formatter)

    stderr_handler = logging.StreamHandler(sys.stderr)
    stderr_handler.setLevel(logging.DEBUG)
    stderr_handler.setFormatter(formatter)

    root_logger = logging.getLogger()
    root_logger.setLevel(logging.DEBUG)
    root_logger.addHandler(file_handler)
    root_logger.addHandler(stderr_handler)

    logger.info("Sidecar 日志系统初始化完成, 日志文件: %s", log_file)


# 文档处理器注册表
# txt 类型复用 MarkdownHandler，纯文本是 Markdown 的子集
HANDLERS = {
    "docx": WordHandler(),
    "xlsx": ExcelHandler(),
    "pptx": PptHandler(),
    "pdf": PdfHandler(),
    "md": MarkdownHandler(),
    "markdown": MarkdownHandler(),
    "txt": MarkdownHandler(),
    "code": CodeHandler(),  # Code Interpreter 代码执行处理器
}

# 文档验证器实例
_validator = DocumentValidator()


def handle_request(request: dict) -> dict:
    """处理文档操作请求

    请求格式:
    {
        "id": "请求唯一ID",
        "action": "generate|read|modify|delete|convert|analyze|ping",
        "type": "docx|xlsx|pptx|pdf|md|health",
        "params": { ... }
    }

    响应格式:
    {
        "id": "请求唯一ID",
        "success": true|false,
        "data": { ... },   # 成功时
        "error": "..."      # 失败时
    }
    """
    request_id = request.get("id", "")
    action = request.get("action", "")
    doc_type = request.get("type", "")
    params = request.get("params", {})

    logger.info("收到请求: id=%s, action=%s, type=%s", request_id, action, doc_type)

    # 健康检查请求，直接返回成功响应
    if action == "ping" or doc_type == "health":
        logger.debug("健康检查请求: id=%s", request_id)
        return {
            "id": request_id,
            "success": True,
            "data": {"status": "ok"},
        }

    # 验证请求，使用 DocumentValidator
    if action == "validate":
        logger.info("验证请求: id=%s, type=%s", request_id, doc_type)
        file_path = params.get("path", "")
        if "input_path" in params and "path" not in params:
            file_path = params["input_path"]
        options = params.get("options", {})
        try:
            result = _validator.validate(file_path, doc_type, options)
            return {
                "id": request_id,
                "success": True,
                "data": result,
            }
        except Exception as e:
            logger.error("验证失败: id=%s, error=%s: %s", request_id, type(e).__name__, e)
            return {
                "id": request_id,
                "success": False,
                "error": f"验证失败: {type(e).__name__}: {e}",
            }

    handler = HANDLERS.get(doc_type)
    if handler is None:
        logger.error("不支持的文档类型: %s", doc_type)
        return {
            "id": request_id,
            "success": False,
            "error": f"不支持的文档类型: {doc_type}",
        }

    action_method = getattr(handler, action, None)
    if action_method is None:
        logger.error("不支持的操作: %s/%s", action, doc_type)
        return {
            "id": request_id,
            "success": False,
            "error": f"不支持的操作: {action}/{doc_type}",
        }

    # 将 Rust 端发送的 input_path 映射为 Python handler 期望的 path
    # 适用于所有需要文件路径的操作（read、modify、convert、analyze 等）
    if "input_path" in params and "path" not in params:
        params["path"] = params["input_path"]

    try:
        result = action_method(params)
        # 检查结果中是否包含错误信息（handler 参数校验失败时返回含 error 键的字典而非抛出异常）
        if isinstance(result, dict) and "error" in result:
            logger.warning("操作返回错误: id=%s, error=%s", request_id, result["error"])
            return {
                "id": request_id,
                "success": False,
                "error": result["error"],
            }
        logger.info("操作执行成功: id=%s, action=%s/%s", request_id, action, doc_type)
        return {
            "id": request_id,
            "success": True,
            "data": result,
        }
    except FileNotFoundError as e:
        logger.error("文件未找到: id=%s, error=%s", request_id, e)
        return {
            "id": request_id,
            "success": False,
            "error": f"文件未找到: {e}",
        }
    except PermissionError as e:
        logger.error("权限不足: id=%s, error=%s", request_id, e)
        return {
            "id": request_id,
            "success": False,
            "error": f"权限不足: {e}",
        }
    except Exception as e:
        logger.error("操作执行失败: id=%s, error=%s: %s", request_id, type(e).__name__, e)
        return {
            "id": request_id,
            "success": False,
            "error": f"{type(e).__name__}: {e}",
            "traceback": traceback.format_exc(),
        }


def main():
    """主循环：从 stdin 读取 JSON 请求，处理并输出到 stdout"""
    # Windows 管道模式下 stdin/stdout 默认使用系统编码（如 GBK/cp936），
    # 而 Rust 端发送 UTF-8 编码的 JSON，编码不匹配会导致 surrogate 字符产生，
    # 引发 UnicodeEncodeError。显式重新配置为 UTF-8 解决此问题。
    sys.stdin.reconfigure(encoding='utf-8')
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')

    setup_logging()
    logger.info("Sidecar 启动, 等待输入...")

    for line in sys.stdin:
        line = line.strip()
        # 移除 UTF-8 BOM（Windows 管道常见问题）
        line = line.lstrip('\ufeff')
        if not line:
            continue
        logger.debug("收到输入: %s", line[:200])
        try:
            request = json.loads(line)
            response = handle_request(request)
        except json.JSONDecodeError as e:
            logger.error("JSON 解析错误: %s", e)
            response = {"id": "", "success": False, "error": f"JSON 解析错误: {e}"}
        except Exception as e:
            logger.error("内部错误: %s: %s", type(e).__name__, e)
            response = {
                "id": "",
                "success": False,
                "error": f"内部错误: {e}",
                "traceback": traceback.format_exc(),
            }

        sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
