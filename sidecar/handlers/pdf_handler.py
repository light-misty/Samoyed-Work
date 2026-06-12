"""PDF 文档处理器
基于 reportlab + pypdf 实现 PDF 文档的读取、转换、分析
精简版：仅支持 read/convert/analyze 操作
"""

import os
import html
import logging


class PdfHandler:
    """PDF 文档处理器（精简版，仅支持 read/convert/analyze）"""

    logger = logging.getLogger(__name__)

    def read(self, params: dict) -> dict:
        """读取 PDF 文档内容

        params:
            path: 文件路径
            pages: 要读取的页码列表（可选，默认读取所有）
        """
        path = params.get("path", "")
        pages = params.get("pages", None)
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 PDF 文档, path=%s", path)

        try:
            import pdfplumber
        except ImportError:
            self.logger.error("read: pdfplumber 未安装，无法读取 PDF")
            return {"error": "pdfplumber 未安装"}

        text_content = []
        with pdfplumber.open(path) as pdf:
            total_pages = len(pdf.pages)
            page_indices = range(total_pages)
            if pages:
                page_indices = [p - 1 for p in pages if 1 <= p <= total_pages]

            for idx in page_indices:
                page = pdf.pages[idx]
                page_text = page.extract_text() or ""
                text_content.append({
                    "page": idx + 1,
                    "text": page_text,
                })

        self.logger.info("read: PDF 文档读取完成, path=%s, 总页数=%d", path, len(text_content))
        return {
            "pages": text_content,
            "total_pages": len(text_content),
        }

    def convert(self, params: dict) -> dict:
        """格式转换

        params:
            path: 源文件路径
            output_path: 输出文件路径（可选）
            format: 目标格式（txt, md, html）
        """
        path = params.get("path", "")
        output_path = params.get("output_path", "")
        target_format = params.get("format", "txt")

        if not path:
            self.logger.error("convert: 缺少源文件路径")
            return {"error": "缺少源文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("convert: 开始格式转换, path=%s, format=%s", path, target_format)

        # 读取 PDF 内容
        try:
            import pdfplumber
        except ImportError:
            self.logger.error("convert: pdfplumber 未安装，无法读取 PDF")
            return {"error": "pdfplumber 未安装"}

        pages_text = []
        with pdfplumber.open(path) as pdf:
            for page in pdf.pages:
                text = page.extract_text() or ""
                pages_text.append(text)

        # 转换为目标格式
        if target_format == "txt":
            content = "\n\n".join(pages_text)
        elif target_format in ("md", "markdown"):
            parts = []
            for i, text in enumerate(pages_text):
                parts.append(f"## 第 {i + 1} 页\n")
                parts.append(text)
                parts.append("")
            content = "\n".join(parts)
        elif target_format == "html":
            content = self._convert_pages_to_html(pages_text)
        else:
            self.logger.error("convert: 不支持的目标格式: %s", target_format)
            return {"error": f"不支持的目标格式: {target_format}"}

        if output_path:
            os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
            with open(output_path, "w", encoding="utf-8") as f:
                f.write(content)
            self.logger.info("convert: 格式转换完成, output_path=%s, format=%s", output_path, target_format)
            return {
                "path": output_path,
                "format": target_format,
                "message": f"已转换为 {target_format} 格式",
            }
        else:
            return {
                "content": content,
                "format": target_format,
            }

    def analyze(self, params: dict) -> dict:
        """分析 PDF 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 PDF 文档, path=%s", path)

        from pypdf import PdfReader

        reader = PdfReader(path)
        total_pages = len(reader.pages)

        # 提取元数据
        meta = reader.metadata
        metadata = {}
        if meta:
            metadata = {
                "title": meta.title or "",
                "author": meta.author or "",
                "subject": meta.subject or "",
                "creator": meta.creator or "",
                "producer": meta.producer or "",
            }

        # 统计文本
        total_chars = 0
        try:
            import pdfplumber
            with pdfplumber.open(path) as pdf:
                for page in pdf.pages:
                    text = page.extract_text() or ""
                    total_chars += len(text)
        except ImportError:
            self.logger.warning("analyze: pdfplumber 未安装，跳过文本统计")

        self.logger.info("analyze: PDF 文档分析完成, path=%s, 总页数=%d", path, total_pages)
        return {
            "file_size": os.path.getsize(path),
            "total_pages": total_pages,
            "total_chars": total_chars,
            "metadata": metadata,
        }

    # ------------------------------------------------------------------ #
    #  格式转换辅助方法
    # ------------------------------------------------------------------ #

    def _convert_pages_to_html(self, pages_text: list[str]) -> str:
        """将 PDF 页面文本转换为 HTML"""
        sections = []
        for i, text in enumerate(pages_text):
            section_lines = [f'  <div class="page">']
            section_lines.append(f"    <h3>第 {i + 1} 页</h3>")
            for line in text.split("\n"):
                if line.strip():
                    section_lines.append(f"    <p>{html.escape(line)}</p>")
            section_lines.append("  </div>")
            sections.append("\n".join(section_lines))

        html_doc = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>PDF Content</title>
  <style>
    body {{ font-family: "Microsoft YaHei", "SimSun", sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }}
    .page {{ margin-bottom: 30px; padding: 15px; border: 1px solid #ddd; border-radius: 4px; }}
    h3 {{ color: #666; border-bottom: 1px solid #eee; padding-bottom: 5px; }}
    p {{ line-height: 1.8; color: #333; margin: 4px 0; }}
  </style>
</head>
<body>
{chr(10).join(sections)}
</body>
</html>"""
        return html_doc
