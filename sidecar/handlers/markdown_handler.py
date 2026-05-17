"""Markdown 文档处理器
实现 Markdown 文档的生成、读取、修改、转换
"""

import os
import re
import logging
from typing import Any


class MarkdownHandler:
    """Markdown (.md) 文档处理器"""

    logger = logging.getLogger(__name__)

    def generate(self, params: dict) -> dict:
        """生成 Markdown 文档

        params:
            path: 输出文件路径
            title: 文档标题
            content: 文档内容
        """
        path = params.get("path", "")
        title = params.get("title", "")
        content = params.get("content", "")

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("generate: 开始生成 Markdown 文档, path=%s", path)

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        lines = []
        if title:
            lines.append(f"# {title}")
            lines.append("")

        if isinstance(content, str):
            lines.append(content)
        elif isinstance(content, list):
            for item in content:
                if isinstance(item, str):
                    lines.append(item)
                    lines.append("")
                elif isinstance(item, dict):
                    block_type = item.get("type", "paragraph")
                    text = item.get("text", "")
                    if block_type == "heading":
                        level = item.get("level", 1)
                        lines.append(f"{'#' * level} {text}")
                    elif block_type == "list":
                        for li in item.get("items", []):
                            lines.append(f"- {li}")
                    elif block_type == "code":
                        lang = item.get("language", "")
                        lines.append(f"```{lang}")
                        lines.append(text)
                        lines.append("```")
                    else:
                        lines.append(text)
                    lines.append("")

        md_content = "\n".join(lines)
        with open(path, "w", encoding="utf-8") as f:
            f.write(md_content)

        self.logger.info("generate: Markdown 文档已生成, path=%s", path)
        return {
            "path": path,
            "message": f"Markdown 文档已生成: {path}",
        }

    def read(self, params: dict) -> dict:
        """读取 Markdown 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 Markdown 文档, path=%s", path)

        with open(path, "r", encoding="utf-8") as f:
            content = f.read()

        # 解析标题结构
        headings = []
        for match in re.finditer(r"^(#{1,6})\s+(.+)$", content, re.MULTILINE):
            level = len(match.group(1))
            text = match.group(2).strip()
            headings.append({"level": level, "text": text})

        self.logger.info("read: Markdown 文档读取完成, path=%s, 标题数=%d", path, len(headings))
        return {
            "content": content,
            "headings": headings,
            "heading_count": len(headings),
            "line_count": content.count("\n") + 1,
            "char_count": len(content),
        }

    def modify(self, params: dict) -> dict:
        """修改 Markdown 文档"""
        path = params.get("path", "")
        operations = params.get("operations", [])
        if not path:
            self.logger.error("modify: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("modify: 开始修改 Markdown 文档, path=%s, 操作数=%d", path, len(operations))

        with open(path, "r", encoding="utf-8") as f:
            content = f.read()

        modified_count = 0

        for op in operations:
            op_type = op.get("type", "")

            if op_type == "replace":
                old_text = op.get("old", "")
                new_text = op.get("new", "")
                if old_text in content:
                    content = content.replace(old_text, new_text)
                    modified_count += 1

            elif op_type == "append":
                text = op.get("text", "")
                content = content.rstrip() + "\n\n" + text
                modified_count += 1

            elif op_type == "prepend":
                text = op.get("text", "")
                content = text + "\n\n" + content.lstrip()
                modified_count += 1

            elif op_type == "insert_after_heading":
                heading_text = op.get("heading", "")
                insert_text = op.get("text", "")
                pattern = re.compile(
                    rf"^(#{1,6}\s+{re.escape(heading_text)})$",
                    re.MULTILINE,
                )
                match = pattern.search(content)
                if match:
                    insert_pos = match.end()
                    content = content[:insert_pos] + "\n\n" + insert_text + content[insert_pos:]
                    modified_count += 1

        with open(path, "w", encoding="utf-8") as f:
            f.write(content)

        self.logger.info("modify: Markdown 文档修改完成, path=%s, 修改数=%d", path, modified_count)
        return {
            "path": path,
            "modified_count": modified_count,
            "message": f"已执行 {modified_count} 项修改",
        }

    def convert(self, params: dict) -> dict:
        """格式转换"""
        self.logger.error("convert: Markdown 格式转换暂未实现")
        return {"error": "Markdown 格式转换暂未实现"}

    def analyze(self, params: dict) -> dict:
        """分析 Markdown 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 Markdown 文档, path=%s", path)

        with open(path, "r", encoding="utf-8") as f:
            content = f.read()

        # 统计
        headings = re.findall(r"^#{1,6}\s+.+$", content, re.MULTILINE)
        code_blocks = re.findall(r"```[\s\S]*?```", content)
        links = re.findall(r"\[([^\]]+)\]\(([^)]+)\)", content)
        images = re.findall(r"!\[([^\]]*)\]\(([^)]+)\)", content)

        self.logger.info("analyze: Markdown 文档分析完成, path=%s, 标题数=%d, 代码块数=%d", path, len(headings), len(code_blocks))
        return {
            "file_size": os.path.getsize(path),
            "char_count": len(content),
            "line_count": content.count("\n") + 1,
            "heading_count": len(headings),
            "code_block_count": len(code_blocks),
            "link_count": len(links),
            "image_count": len(images),
        }
