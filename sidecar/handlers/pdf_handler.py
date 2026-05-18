"""PDF 文档处理器
基于 reportlab 实现 PDF 生成，基于 pdfkit 实现 HTML 转 PDF
"""

import os
import html
import logging
from typing import Any


class PdfHandler:
    """PDF (.pdf) 文档处理器"""

    logger = logging.getLogger(__name__)

    def generate(self, params: dict) -> dict:
        """生成 PDF 文档

        params:
            path: 输出文件路径
            title: 文档标题
            content: 文档内容
            author: 作者
        """
        path = params.get("path", "")
        title = params.get("title", "")
        content = params.get("content", "")
        author = params.get("author", "")

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("generate: 开始生成 PDF 文档, path=%s", path)

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        try:
            from reportlab.lib.pagesizes import A4
            from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
            from reportlab.lib.units import cm
            from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer
            from reportlab.pdfbase import pdfmetrics
            from reportlab.pdfbase.ttfonts import TTFont

            # 注册中文字体（尝试使用系统字体）
            font_paths = [
                "C:/Windows/Fonts/msyh.ttc",
                "C:/Windows/Fonts/simsun.ttc",
                "C:/Windows/Fonts/simhei.ttf",
            ]
            font_name = "Helvetica"
            for fp in font_paths:
                if os.path.exists(fp):
                    try:
                        pdfmetrics.registerFont(TTFont("ChineseFont", fp))
                        font_name = "ChineseFont"
                        break
                    except Exception:
                        continue

            doc = SimpleDocTemplate(path, pagesize=A4)
            styles = getSampleStyleSheet()

            # 自定义标题样式
            title_style = ParagraphStyle(
                "CustomTitle",
                parent=styles["Title"],
                fontName=font_name,
                fontSize=24,
                spaceAfter=30,
            )

            # 自定义正文样式
            body_style = ParagraphStyle(
                "CustomBody",
                parent=styles["Normal"],
                fontName=font_name,
                fontSize=12,
                leading=20,
                spaceAfter=10,
            )

            elements = []

            # 添加标题（Paragraph 使用 XML 标记语法，需转义特殊字符）
            if title:
                elements.append(Paragraph(html.escape(title), title_style))
                elements.append(Spacer(1, 1 * cm))

            # 添加内容
            if isinstance(content, str):
                for line in content.split("\n"):
                    if line.strip():
                        elements.append(Paragraph(html.escape(line), body_style))
            elif isinstance(content, list):
                for item in content:
                    if isinstance(item, str):
                        elements.append(Paragraph(html.escape(item), body_style))
                    elif isinstance(item, dict):
                        text = item.get("text", "")
                        style_type = item.get("style", "body")
                        if style_type == "heading":
                            elements.append(Paragraph(html.escape(text), title_style))
                        else:
                            elements.append(Paragraph(html.escape(text), body_style))

            # 设置作者
            if author:
                doc.author = author

            doc.build(elements)

            self.logger.info("generate: PDF 文档已生成, path=%s", path)
            return {
                "path": path,
                "message": f"PDF 文档已生成: {path}",
            }

        except ImportError:
            self.logger.error("generate: reportlab 未安装，无法生成 PDF")
            return {"error": "reportlab 未安装，无法生成 PDF"}

    def read(self, params: dict) -> dict:
        """读取 PDF 文档（提取文本）"""
        path = params.get("path", "")
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 PDF 文档, path=%s", path)

        try:
            import fitz  # PyMuPDF
            doc = fitz.open(path)
            pages = []
            for page in doc:
                pages.append({
                    "page_number": page.number + 1,
                    "text": page.get_text(),
                })
            doc.close()
            self.logger.info("read: PDF 文档读取完成, path=%s, 页数=%d", path, len(pages))
            return {
                "pages": pages,
                "page_count": len(pages),
            }
        except ImportError:
            # 回退方案：使用 pdfminer
            try:
                from pdfminer.high_level import extract_text
                text = extract_text(path)
                self.logger.info("read: PDF 文档读取完成(pdfminer), path=%s", path)
                return {
                    "text": text,
                    "page_count": text.count("\f") + 1,
                }
            except ImportError:
                self.logger.error("read: 未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）")
                return {
                    "text": "",
                    "error": "未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）",
                    "page_count": 0,
                }

    def modify(self, params: dict) -> dict:
        """修改 PDF 文档（PDF 不易直接修改，建议转换为其他格式后修改）"""
        self.logger.error("modify: PDF 格式不支持直接修改")
        return {"error": "PDF 格式不支持直接修改，建议转换为 Word 后修改"}

    def convert(self, params: dict) -> dict:
        """格式转换"""
        self.logger.error("convert: PDF 格式转换暂未实现")
        return {"error": "PDF 格式转换暂未实现"}

    def analyze(self, params: dict) -> dict:
        """分析 PDF 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 PDF 文档, path=%s", path)

        try:
            import fitz
            doc = fitz.open(path)
            info = {
                "file_size": os.path.getsize(path),
                "page_count": len(doc),
                "metadata": doc.metadata,
            }
            doc.close()
            self.logger.info("analyze: PDF 文档分析完成, path=%s, 页数=%d", path, info["page_count"])
            return info
        except ImportError:
            self.logger.error("analyze: 未安装 PyMuPDF")
            return {
                "file_size": os.path.getsize(path),
                "page_count": 0,
                "error": "未安装 PyMuPDF",
            }
