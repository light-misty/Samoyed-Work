"""PDF 文档处理器
基于 reportlab 实现 PDF 生成，基于 pdfkit 实现 HTML 转 PDF
支持高级操作：合并、拆分、旋转、水印、加密（依赖 pypdf）
"""

import os
import io
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
            subscripts: 下标列表 [{text: "2", position: 1}]，position 为字符插入位置
            superscripts: 上标列表 [{text: "2", position: 3}]，position 为字符插入位置
            pageSize: 页面尺寸 "letter" | "a4"（默认 a4）
        """
        path = params.get("path", "")
        title = params.get("title", "")
        content = params.get("content", "")
        author = params.get("author", "")
        # 下标和上标参数
        subscripts = params.get("subscripts", [])
        superscripts = params.get("superscripts", [])
        # 页面尺寸参数
        page_size_name = params.get("pageSize", "a4").lower()

        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("generate: 开始生成 PDF 文档, path=%s", path)

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        try:
            from reportlab.lib.pagesizes import A4, letter
            from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
            from reportlab.lib.units import cm
            from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer
        except ImportError:
            self.logger.error("generate: reportlab 未安装，无法生成 PDF")
            return {"error": "reportlab 未安装，无法生成 PDF"}

        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

        # 根据参数选择页面尺寸
        page_size = letter if page_size_name == "letter" else A4

        doc = SimpleDocTemplate(path, pagesize=page_size)
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
            # 标题不支持下标上标，直接转义
            elements.append(Paragraph(html.escape(title), title_style))
            elements.append(Spacer(1, 1 * cm))

        # 判断是否需要应用下标/上标
        has_sub_super = bool(subscripts or superscripts)

        # 添加内容
        if isinstance(content, str):
            if has_sub_super:
                # 有下标/上标时，先对整个内容应用标签，再按行分割
                processed = self._apply_sub_super(content, subscripts, superscripts)
                for line in processed.split("\n"):
                    if line.strip():
                        elements.append(Paragraph(line, body_style))
            else:
                for line in content.split("\n"):
                    if line.strip():
                        # 检查内容是否已包含 reportlab XML 标签，直接渲染
                        if "<sub>" in line or "<super>" in line:
                            elements.append(Paragraph(line, body_style))
                        else:
                            elements.append(Paragraph(html.escape(line), body_style))
        elif isinstance(content, list):
            for item in content:
                if isinstance(item, str):
                    if has_sub_super:
                        processed = self._apply_sub_super(item, subscripts, superscripts)
                        elements.append(Paragraph(processed, body_style))
                    else:
                        if "<sub>" in item or "<super>" in item:
                            elements.append(Paragraph(item, body_style))
                        else:
                            elements.append(Paragraph(html.escape(item), body_style))
                elif isinstance(item, dict):
                    text = item.get("text", "")
                    style_type = item.get("style", "body")
                    if has_sub_super:
                        processed = self._apply_sub_super(text, subscripts, superscripts)
                        target_style = title_style if style_type == "heading" else body_style
                        elements.append(Paragraph(processed, target_style))
                    else:
                        if "<sub>" in text or "<super>" in text:
                            target_style = title_style if style_type == "heading" else body_style
                            elements.append(Paragraph(text, target_style))
                        else:
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
            # 回退方案：使用 pdfminer，统一返回结构
            try:
                from pdfminer.high_level import extract_text
                text = extract_text(path)
                # 将文本按换页符分割为页面，保持与 PyMuPDF 一致的返回结构
                page_texts = text.split("\f")
                pages = []
                for i, page_text in enumerate(page_texts):
                    if page_text.strip():
                        pages.append({
                            "page_number": i + 1,
                            "text": page_text,
                        })
                self.logger.info("read: PDF 文档读取完成(pdfminer), path=%s, 页数=%d", path, len(pages))
                return {
                    "pages": pages,
                    "page_count": len(pages),
                }
            except ImportError:
                self.logger.error("read: 未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）")
                return {
                    "pages": [],
                    "page_count": 0,
                    "error": "未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）",
                }

    def modify(self, params: dict) -> dict:
        """修改 PDF 文档，支持高级操作

        支持两种调用格式：

        格式1 - 直接操作（向后兼容）:
            type: 操作类型（merge/split/rotate/addWatermark/encrypt）
            其他参数根据操作类型而定

        格式2 - operations 数组（与其他 Handler 一致，由 Rust 端调用）:
            path: 源 PDF 文件路径
            operations: 操作列表
                - {type: "merge", files: [...], outputPath: "..."}
                - {type: "split", ranges: ["1-5", "6-10"], outputDir: "..."}
                - {type: "rotate", pages: [1,2,3], angle: 90, outputPath: "..."}
                - {type: "addWatermark", text: "...", outputPath: "..."}
                  或 {type: "addWatermark", image: "...", outputPath: "..."}
                - {type: "encrypt", userPassword: "...", ownerPassword: "...", outputPath: "..."}
        """
        # 判断是 operations 数组格式还是直接操作格式
        operations = params.get("operations", [])
        if operations and isinstance(operations, list):
            # operations 数组格式：逐个执行操作，返回最后一个操作的结果
            path = params.get("path", "")
            last_result = None
            executed_count = 0
            for op in operations:
                # 将操作参数合并到顶层，同时传入 path
                op_params = dict(op)
                op_params["path"] = op_params.get("path", path)
                # camelCase 转 snake_case 的路径映射
                if "outputPath" in op_params and "output_path" not in op_params:
                    op_params["output_path"] = op_params.pop("outputPath")
                if "outputDir" in op_params and "output_dir" not in op_params:
                    op_params["output_dir"] = op_params.pop("outputDir")
                result = self._execute_single_operation(op_params)
                if isinstance(result, dict) and "error" in result:
                    # 操作失败，立即返回错误
                    return result
                last_result = result
                executed_count += 1
            if last_result is None:
                return {"error": "没有可执行的操作"}
            # 在结果中添加执行计数
            if isinstance(last_result, dict):
                last_result["executed_count"] = executed_count
            return last_result
        else:
            # 直接操作格式（向后兼容）
            return self._execute_single_operation(params)

    def _execute_single_operation(self, params: dict) -> dict:
        """执行单个 PDF 操作

        Args:
            params: 操作参数，必须包含 type 字段
        """
        operation = params.get("type", "")

        if not operation:
            self.logger.error("modify: 缺少操作类型")
            return {"error": "PDF 格式不支持直接修改，请指定操作类型: merge/split/rotate/addWatermark/encrypt"}

        self.logger.info("modify: 执行 PDF 高级操作, type=%s", operation)

        if operation == "merge":
            return self._merge_pdfs(
                params.get("files", []),
                params.get("output_path", ""),
            )
        elif operation == "split":
            return self._split_pdf(
                params.get("path", ""),
                params.get("ranges", []),
                params.get("output_dir", ""),
            )
        elif operation == "rotate":
            return self._rotate_pages(
                params.get("path", ""),
                params.get("pages", []),
                params.get("angle", 90),
                params.get("output_path", ""),
            )
        elif operation == "addWatermark":
            # 根据参数判断是文字水印还是图片水印
            if "image" in params:
                return self._add_image_watermark(
                    params.get("path", ""),
                    params.get("image", ""),
                    params.get("output_path", ""),
                )
            else:
                return self._add_text_watermark(
                    params.get("path", ""),
                    params.get("text", ""),
                    params.get("output_path", ""),
                )
        elif operation == "encrypt":
            return self._encrypt_pdf(
                params.get("path", ""),
                params.get("userPassword", ""),
                params.get("ownerPassword", ""),
                params.get("output_path", ""),
            )
        else:
            self.logger.error("modify: 不支持的操作类型: %s", operation)
            return {"error": f"不支持的操作类型: {operation}，支持: merge/split/rotate/addWatermark/encrypt"}

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

        # PDF 只支持文本提取类转换，不支持转为二进制格式
        supported_formats = ("txt", "md", "markdown", "html")
        if target_format not in supported_formats:
            self.logger.error("convert: 不支持的目标格式: %s，PDF 仅支持转为 txt/md/html", target_format)
            return {"error": f"不支持的目标格式: {target_format}，PDF 仅支持转为 txt/md/html"}

        self.logger.info("convert: 开始格式转换, path=%s, format=%s", path, target_format)

        # 提取 PDF 各页文本
        pages = self._extract_pages(path)
        if pages is None:
            return {"error": "未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）"}

        # 根据目标格式生成内容
        if target_format == "txt":
            content = self._convert_to_txt(pages)
        elif target_format in ("md", "markdown"):
            content = self._convert_to_md(pages)
        elif target_format == "html":
            content = self._convert_to_html(pages)

        # 写入输出文件或直接返回内容
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

    def _extract_pages(self, path: str) -> list[dict] | None:
        """提取 PDF 各页文本，优先使用 PyMuPDF，回退到 pdfminer

        返回:
            [{"page_number": 1, "text": "..."}, ...] 或 None（库未安装时）
        """
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
            return pages
        except ImportError:
            # 回退方案：使用 pdfminer
            try:
                from pdfminer.high_level import extract_text
                text = extract_text(path)
                # 将文本按换页符分割为页面
                page_texts = text.split("\f")
                pages = []
                for i, page_text in enumerate(page_texts):
                    if page_text.strip():
                        pages.append({
                            "page_number": i + 1,
                            "text": page_text,
                        })
                return pages
            except ImportError:
                self.logger.error("convert: 未安装 PDF 读取库（PyMuPDF 或 pdfminer.six）")
                return None

    def _convert_to_txt(self, pages: list[dict]) -> str:
        """将页面文本列表转换为纯文本格式"""
        parts = []
        for page in pages:
            parts.append(page["text"])
        return "\n\n".join(parts)

    def _convert_to_md(self, pages: list[dict]) -> str:
        """将页面文本列表转换为 Markdown 格式，每页用 ## 标题分隔"""
        lines = []
        for page in pages:
            page_num = page["page_number"]
            text = page["text"].strip()
            if not text:
                continue
            lines.append(f"## 第 {page_num} 页")
            lines.append("")
            lines.append(text)
            lines.append("")
        return "\n".join(lines)

    def _convert_to_html(self, pages: list[dict]) -> str:
        """将页面文本列表转换为 HTML 格式，每页用 section 标签包裹，段落用 p 标签"""
        sections = []
        for page in pages:
            page_num = page["page_number"]
            text = page["text"].strip()
            if not text:
                continue
            # 将文本按空行分割为段落
            paragraphs = text.split("\n\n")
            para_tags = []
            for para in paragraphs:
                para = para.strip()
                if para:
                    # 将段落内换行替换为 <br>
                    para_html = html.escape(para).replace("\n", "<br>")
                    para_tags.append(f"<p>{para_html}</p>")
            section_content = "\n    ".join(para_tags)
            sections.append(
                f'<section data-page="{page_num}">\n'
                f"    {section_content}\n"
                f"</section>"
            )
        body = "\n\n".join(sections)
        return (
            "<!DOCTYPE html>\n"
            "<html lang=\"zh-CN\">\n"
            "<head>\n"
            '  <meta charset="UTF-8">\n'
            "  <title>PDF 转换结果</title>\n"
            "</head>\n"
            "<body>\n"
            f"{body}\n"
            "</body>\n"
            "</html>"
        )

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

    def _apply_sub_super(self, text: str, subscripts: list, superscripts: list) -> str:
        """在下标和上标位置插入 reportlab XML 标签，同时正确转义非标签文本

        将 subscripts/superscripts 列表中的条目按 position 插入到文本中，
        非标签部分的特殊字符会被转义，<sub>/<super> 标签本身保留供 reportlab Paragraph 解析。

        Args:
            text: 原始文本
            subscripts: 下标列表 [{text: "2", position: 1}]
            superscripts: 上标列表 [{text: "2", position: 3}]

        Returns:
            处理后的文本，包含 <sub>/<super> 标签
        """
        # 合并所有插入点
        insertions = []
        for sub in subscripts:
            insertions.append((sub.get("position", 0), "sub", sub.get("text", "")))
        for sup in superscripts:
            insertions.append((sup.get("position", 0), "super", sup.get("text", "")))

        if not insertions:
            return html.escape(text)

        # 按位置正序排列，依次分割文本
        insertions.sort(key=lambda x: x[0])

        # 分割文本，转义各部分，然后用标签重新连接
        parts = []
        last_pos = 0
        for pos, tag_type, tag_text in insertions:
            # 转义标签位置前的文本
            parts.append(html.escape(text[last_pos:pos]))
            # 插入标签（标签本身不转义，标签内文本需转义）
            parts.append(f"<{tag_type}>{html.escape(tag_text)}</{tag_type}>")
            last_pos = pos
        # 转义最后一部分文本
        parts.append(html.escape(text[last_pos:]))

        return "".join(parts)

    def _merge_pdfs(self, files: list, output_path: str) -> dict:
        """合并多个 PDF 文件

        Args:
            files: 要合并的 PDF 文件路径列表
            output_path: 输出文件路径

        Returns:
            操作结果字典
        """
        try:
            from pypdf import PdfWriter, PdfReader
        except ImportError:
            self.logger.error("_merge_pdfs: pypdf 未安装")
            return {"error": "需要安装 pypdf 库: pip install pypdf"}

        if not files:
            self.logger.error("_merge_pdfs: 缺少要合并的文件列表")
            return {"error": "缺少要合并的文件列表"}

        if not output_path:
            self.logger.error("_merge_pdfs: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("_merge_pdfs: 开始合并 %d 个 PDF 文件", len(files))

        writer = PdfWriter()
        merged_count = 0
        for file_path in files:
            if not os.path.exists(file_path):
                self.logger.error("_merge_pdfs: 文件不存在: %s", file_path)
                return {"error": f"文件不存在: {file_path}"}
            reader = PdfReader(file_path)
            for page in reader.pages:
                writer.add_page(page)
            merged_count += 1

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as f:
            writer.write(f)

        self.logger.info("_merge_pdfs: 合并完成, output_path=%s, 文件数=%d", output_path, merged_count)
        return {
            "path": output_path,
            "message": f"已合并 {merged_count} 个 PDF 文件",
        }

    def _split_pdf(self, path: str, ranges: list, output_dir: str) -> dict:
        """按页码范围拆分 PDF 文件

        Args:
            path: 源 PDF 文件路径
            ranges: 页码范围列表，如 ["1-5", "6-10"]
            output_dir: 输出目录（默认为源文件所在目录）

        Returns:
            操作结果字典，包含生成的文件列表
        """
        try:
            from pypdf import PdfWriter, PdfReader
        except ImportError:
            self.logger.error("_split_pdf: pypdf 未安装")
            return {"error": "需要安装 pypdf 库: pip install pypdf"}

        if not path or not os.path.exists(path):
            self.logger.error("_split_pdf: 源文件不存在: %s", path)
            return {"error": "源文件不存在"}

        if not ranges:
            self.logger.error("_split_pdf: 缺少拆分页码范围")
            return {"error": "缺少拆分页码范围"}

        # 默认输出到源文件所在目录
        if not output_dir:
            output_dir = os.path.dirname(path) or "."

        os.makedirs(output_dir, exist_ok=True)

        reader = PdfReader(path)
        total_pages = len(reader.pages)
        base_name = os.path.splitext(os.path.basename(path))[0]
        output_files = []

        self.logger.info("_split_pdf: 开始拆分, path=%s, ranges=%s, 总页数=%d", path, ranges, total_pages)

        for range_str in ranges:
            # 解析页码范围，格式: "1-5" 表示第1到5页
            parts = range_str.split("-")
            if len(parts) != 2:
                self.logger.error("_split_pdf: 无效的页码范围: %s", range_str)
                return {"error": f"无效的页码范围: {range_str}，格式应为 '起始页-结束页'"}

            try:
                start = int(parts[0].strip())
                end = int(parts[1].strip())
            except ValueError:
                self.logger.error("_split_pdf: 页码范围包含非数字: %s", range_str)
                return {"error": f"页码范围包含非数字: {range_str}"}

            # 校验页码范围有效性
            if start < 1 or end < 1 or start > end:
                self.logger.error("_split_pdf: 无效的页码范围: %s", range_str)
                return {"error": f"无效的页码范围: {range_str}，起始页和结束页须为正整数且起始页不大于结束页"}
            if start > total_pages:
                self.logger.error("_split_pdf: 起始页超出范围: %s (总页数=%d)", range_str, total_pages)
                return {"error": f"起始页超出范围: {range_str}，文档共 {total_pages} 页"}

            writer = PdfWriter()
            # 将页码转为0索引，逐页添加
            actual_end = min(end, total_pages)
            for i in range(start - 1, actual_end):
                writer.add_page(reader.pages[i])

            # 输出文件名: 原文件名_1-5.pdf
            output_name = f"{base_name}_{range_str}.pdf"
            output_path = os.path.join(output_dir, output_name)
            with open(output_path, "wb") as f:
                writer.write(f)
            output_files.append(output_path)

        self.logger.info("_split_pdf: 拆分完成, 生成 %d 个文件", len(output_files))
        return {
            "files": output_files,
            "message": f"已拆分为 {len(output_files)} 个文件",
        }

    def _rotate_pages(self, path: str, pages: list, angle: int, output_path: str) -> dict:
        """旋转 PDF 指定页面

        Args:
            path: 源 PDF 文件路径
            pages: 要旋转的页码列表（1索引），如 [1, 2, 3]
            angle: 旋转角度（90的倍数），如 90, 180, 270
            output_path: 输出文件路径（默认覆盖源文件）

        Returns:
            操作结果字典
        """
        try:
            from pypdf import PdfWriter, PdfReader
        except ImportError:
            self.logger.error("_rotate_pages: pypdf 未安装")
            return {"error": "需要安装 pypdf 库: pip install pypdf"}

        if not path or not os.path.exists(path):
            self.logger.error("_rotate_pages: 源文件不存在: %s", path)
            return {"error": "源文件不存在"}

        if not pages:
            self.logger.error("_rotate_pages: 缺少要旋转的页码列表")
            return {"error": "缺少要旋转的页码列表"}

        # 默认覆盖源文件
        if not output_path:
            output_path = path

        self.logger.info("_rotate_pages: 开始旋转, path=%s, pages=%s, angle=%d", path, pages, angle)

        reader = PdfReader(path)
        writer = PdfWriter()
        rotated_count = 0

        for i, page in enumerate(reader.pages):
            # 页码为1索引，判断当前页是否在旋转列表中
            if (i + 1) in pages:
                page.rotate(angle)
                rotated_count += 1
            writer.add_page(page)

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as f:
            writer.write(f)

        self.logger.info("_rotate_pages: 旋转完成, 旋转了 %d 个页面", rotated_count)
        return {
            "path": output_path,
            "message": f"已旋转 {rotated_count} 个页面，角度: {angle} 度",
        }

    def _add_text_watermark(self, path: str, text: str, output_path: str) -> dict:
        """添加文字水印

        使用 reportlab 生成水印 PDF 页面，再用 pypdf 叠加到每一页。
        水印文字为半透明灰色，45度旋转。

        Args:
            path: 源 PDF 文件路径
            text: 水印文字内容
            output_path: 输出文件路径（默认覆盖源文件）

        Returns:
            操作结果字典
        """
        try:
            from pypdf import PdfWriter, PdfReader
        except ImportError:
            self.logger.error("_add_text_watermark: pypdf 未安装")
            return {"error": "需要安装 pypdf 库: pip install pypdf"}

        try:
            from reportlab.pdfgen import canvas
            from reportlab.lib.colors import Color
        except ImportError:
            self.logger.error("_add_text_watermark: reportlab 未安装")
            return {"error": "需要安装 reportlab 库: pip install reportlab"}

        if not path or not os.path.exists(path):
            self.logger.error("_add_text_watermark: 源文件不存在: %s", path)
            return {"error": "源文件不存在"}

        if not text:
            self.logger.error("_add_text_watermark: 缺少水印文字")
            return {"error": "缺少水印文字"}

        # 默认覆盖源文件
        if not output_path:
            output_path = path

        self.logger.info("_add_text_watermark: 开始添加文字水印, path=%s, text=%s", path, text)

        # 注册中文字体，以支持中文水印
        from handlers.font_utils import register_chinese_font
        font_name = register_chinese_font()

        reader = PdfReader(path)
        writer = PdfWriter()

        for page in reader.pages:
            # 获取当前页面尺寸，为每一页生成对应尺寸的水印
            page_width = float(page.mediabox.width)
            page_height = float(page.mediabox.height)

            # 使用 reportlab 创建水印 PDF 页面
            packet = io.BytesIO()
            can = canvas.Canvas(packet, pagesize=(page_width, page_height))
            can.saveState()
            can.setFont(font_name, 40)
            # 半透明灰色水印
            can.setFillColor(Color(0.75, 0.75, 0.75, alpha=0.3))
            # 移动到页面中心并旋转45度
            can.translate(page_width / 2, page_height / 2)
            can.rotate(45)
            can.drawCentredString(0, 0, text)
            can.restoreState()
            can.save()

            # 将水印页面叠加到当前页面
            packet.seek(0)
            watermark_reader = PdfReader(packet)
            watermark_page = watermark_reader.pages[0]
            page.merge_page(watermark_page)
            writer.add_page(page)

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as f:
            writer.write(f)

        self.logger.info("_add_text_watermark: 水印添加完成, output_path=%s", output_path)
        return {
            "path": output_path,
            "message": f"已添加文字水印: {text}",
        }

    def _add_image_watermark(self, path: str, image_path: str, output_path: str) -> dict:
        """添加图片水印

        使用 reportlab 将图片绘制到 PDF 页面，再用 pypdf 叠加到每一页。

        Args:
            path: 源 PDF 文件路径
            image_path: 水印图片文件路径
            output_path: 输出文件路径（默认覆盖源文件）

        Returns:
            操作结果字典
        """
        try:
            from pypdf import PdfWriter, PdfReader
        except ImportError:
            self.logger.error("_add_image_watermark: pypdf 未安装")
            return {"error": "需要安装 pypdf 库: pip install pypdf"}

        try:
            from reportlab.pdfgen import canvas
            from reportlab.lib.colors import Color
        except ImportError:
            self.logger.error("_add_image_watermark: reportlab 未安装")
            return {"error": "需要安装 reportlab 库: pip install reportlab"}

        if not path or not os.path.exists(path):
            self.logger.error("_add_image_watermark: 源文件不存在: %s", path)
            return {"error": "源文件不存在"}

        if not image_path or not os.path.exists(image_path):
            self.logger.error("_add_image_watermark: 水印图片不存在: %s", image_path)
            return {"error": "水印图片不存在"}

        # 默认覆盖源文件
        if not output_path:
            output_path = path

        self.logger.info("_add_image_watermark: 开始添加图片水印, path=%s, image=%s", path, image_path)

        reader = PdfReader(path)
        writer = PdfWriter()

        for page in reader.pages:
            page_width = float(page.mediabox.width)
            page_height = float(page.mediabox.height)

            # 使用 reportlab 创建带图片的水印 PDF 页面
            packet = io.BytesIO()
            can = canvas.Canvas(packet, pagesize=(page_width, page_height))
            can.saveState()
            # 设置全局透明度
            can.setFillAlpha(0.3)
            # 图片居中放置，宽度为页面宽度的50%
            img_width = page_width * 0.5
            img_height = page_height * 0.5
            x = (page_width - img_width) / 2
            y = (page_height - img_height) / 2
            can.drawImage(image_path, x, y, width=img_width, height=img_height, mask="auto")
            can.restoreState()
            can.save()

            # 将水印页面叠加到当前页面
            packet.seek(0)
            watermark_reader = PdfReader(packet)
            watermark_page = watermark_reader.pages[0]
            page.merge_page(watermark_page)
            writer.add_page(page)

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as f:
            writer.write(f)

        self.logger.info("_add_image_watermark: 图片水印添加完成, output_path=%s", output_path)
        return {
            "path": output_path,
            "message": "已添加图片水印",
        }

    def _encrypt_pdf(self, path: str, user_password: str, owner_password: str, output_path: str) -> dict:
        """加密 PDF 文件

        Args:
            path: 源 PDF 文件路径
            user_password: 用户密码（打开文档需要）
            owner_password: 所有者密码（权限控制）
            output_path: 输出文件路径（默认覆盖源文件）

        Returns:
            操作结果字典
        """
        try:
            from pypdf import PdfWriter, PdfReader
        except ImportError:
            self.logger.error("_encrypt_pdf: pypdf 未安装")
            return {"error": "需要安装 pypdf 库: pip install pypdf"}

        if not path or not os.path.exists(path):
            self.logger.error("_encrypt_pdf: 源文件不存在: %s", path)
            return {"error": "源文件不存在"}

        # 默认覆盖源文件
        if not output_path:
            output_path = path

        self.logger.info("_encrypt_pdf: 开始加密, path=%s", path)

        reader = PdfReader(path)
        writer = PdfWriter()

        for page in reader.pages:
            writer.add_page(page)

        # 使用 pypdf 加密
        writer.encrypt(user_password, owner_password)

        os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
        with open(output_path, "wb") as f:
            writer.write(f)

        self.logger.info("_encrypt_pdf: 加密完成, output_path=%s", output_path)
        return {
            "path": output_path,
            "message": "PDF 已加密",
        }
