"""DocAgent Python Sidecar
文档处理引擎，通过 stdin/stdout JSON 协议与 Rust 后端通信
支持 Word、Excel、PPT、PDF、Markdown 等文档的生成、读取、修改、转换
"""

import sys
import json
import traceback
from typing import Any

from handlers.word_handler import WordHandler
from handlers.excel_handler import ExcelHandler
from handlers.ppt_handler import PptHandler
from handlers.pdf_handler import PdfHandler
from handlers.markdown_handler import MarkdownHandler

# 文档处理器注册表
HANDLERS = {
    "docx": WordHandler(),
    "xlsx": ExcelHandler(),
    "pptx": PptHandler(),
    "pdf": PdfHandler(),
    "md": MarkdownHandler(),
    "markdown": MarkdownHandler(),
}


def handle_request(request: dict) -> dict:
    """处理文档操作请求

    请求格式:
    {
        "id": "请求唯一ID",
        "action": "generate|read|modify|delete|convert|analyze",
        "type": "docx|xlsx|pptx|pdf|md",
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

    # 查找对应的处理器
    handler = HANDLERS.get(doc_type)
    if handler is None:
        return {
            "id": request_id,
            "success": False,
            "error": f"不支持的文档类型: {doc_type}",
        }

    # 查找对应的操作方法
    action_method = getattr(handler, action, None)
    if action_method is None:
        return {
            "id": request_id,
            "success": False,
            "error": f"不支持的操作: {action}/{doc_type}",
        }

    # 执行操作
    try:
        result = action_method(params)
        return {
            "id": request_id,
            "success": True,
            "data": result,
        }
    except FileNotFoundError as e:
        return {
            "id": request_id,
            "success": False,
            "error": f"文件未找到: {e}",
        }
    except PermissionError as e:
        return {
            "id": request_id,
            "success": False,
            "error": f"权限不足: {e}",
        }
    except Exception as e:
        return {
            "id": request_id,
            "success": False,
            "error": f"{type(e).__name__}: {e}",
            "traceback": traceback.format_exc(),
        }


def main():
    """主循环：从 stdin 读取 JSON 请求，处理并输出到 stdout"""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            response = handle_request(request)
        except json.JSONDecodeError as e:
            response = {"id": "", "success": False, "error": f"JSON 解析错误: {e}"}
        except Exception as e:
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
