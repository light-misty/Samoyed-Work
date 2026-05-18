"""PPT 文档处理器
基于 python-pptx 实现 PPT 文档的生成、读取、修改
"""

import os
import logging
from typing import Any

from pptx import Presentation
from pptx.util import Inches, Pt, Emu
from pptx.enum.text import PP_ALIGN


class PptHandler:
    """PowerPoint (.pptx) 文档处理器"""

    logger = logging.getLogger(__name__)

    def generate(self, params: dict) -> dict:
        """生成 PPT 文档

        params:
            path: 输出文件路径
            slides: 幻灯片列表
                [{"title": "...", "content": "...", "layout": "title_slide"}]
            content: 文档内容（当 slides 为空时，从 content 构建）
            title: 文档标题（当 slides 为空时，作为标题幻灯片的标题）
        """
        path = params.get("path", "")
        slides = params.get("slides", [])
        content = params.get("content", "")
        title = params.get("title", "")
        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        # 当 slides 为空但 content 非空时，从 content 构建默认幻灯片
        if not slides and content:
            self.logger.info("generate: slides 为空，从 content 参数构建默认幻灯片")
            # 如果有标题，先创建一张标题幻灯片，再创建内容幻灯片
            if title:
                slides.append({"title": title, "layout": "title_slide"})
            # 将 content 按段落拆分为多张幻灯片
            paragraphs = [p.strip() for p in content.split("\n") if p.strip()]
            # 将段落分组，每组最多 5 个段落作为一张幻灯片
            chunk_size = 5
            for i in range(0, len(paragraphs), chunk_size):
                chunk = paragraphs[i:i + chunk_size]
                slide_title = title if i == 0 and not title else f"第 {i // chunk_size + 1} 页"
                slides.append({
                    "title": slide_title,
                    "content": "\n".join(chunk),
                    "layout": "title_slide",
                })

        self.logger.info("generate: 开始生成 PPT 文档, path=%s, 幻灯片数=%d", path, len(slides))

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        prs = Presentation()

        for slide_info in slides:
            title = slide_info.get("title", "")
            content = slide_info.get("content", "")
            layout_name = slide_info.get("layout", "title_slide")

            # 选择布局
            layout_idx = 0
            for i, layout in enumerate(prs.slide_layouts):
                if layout_name.lower() in layout.name.lower():
                    layout_idx = i
                    break

            slide_layout = prs.slide_layouts[layout_idx]
            slide = prs.slides.add_slide(slide_layout)

            # 设置标题
            if slide.shapes.title:
                slide.shapes.title.text = title

            # 设置内容
            if content and len(slide.placeholders) > 1:
                for placeholder in slide.placeholders:
                    if placeholder.placeholder_format.idx == 1:
                        placeholder.text = content
                        break

        prs.save(path)
        self.logger.info("generate: PPT 文档已生成, path=%s, 幻灯片数=%d", path, len(slides))
        return {
            "path": path,
            "slide_count": len(slides),
            "message": f"PPT 文档已生成: {path}",
        }

    def read(self, params: dict) -> dict:
        """读取 PPT 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 PPT 文档, path=%s", path)

        prs = Presentation(path)
        slides = []
        for slide in prs.slides:
            slide_info = {
                "shapes": [],
            }
            for shape in slide.shapes:
                shape_info = {
                    "name": shape.name,
                    "type": str(shape.shape_type),
                }
                if shape.has_text_frame:
                    texts = []
                    for para in shape.text_frame.paragraphs:
                        texts.append(para.text)
                    shape_info["text"] = "\n".join(texts)
                slide_info["shapes"].append(shape_info)
            slides.append(slide_info)

        self.logger.info("read: PPT 文档读取完成, path=%s, 幻灯片数=%d", path, len(slides))
        return {
            "slides": slides,
            "slide_count": len(slides),
        }

    def modify(self, params: dict) -> dict:
        """修改 PPT 文档"""
        path = params.get("path", "")
        operations = params.get("operations", [])
        if not path:
            self.logger.error("modify: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("modify: 开始修改 PPT 文档, path=%s, 操作数=%d", path, len(operations))

        prs = Presentation(path)
        modified_count = 0

        for op in operations:
            op_type = op.get("type", "")

            if op_type == "add_slide":
                title = op.get("title", "")
                content = op.get("content", "")
                layout_idx = op.get("layout_index", 1)
                if layout_idx < len(prs.slide_layouts):
                    slide = prs.slides.add_slide(prs.slide_layouts[layout_idx])
                    if slide.shapes.title:
                        slide.shapes.title.text = title
                    modified_count += 1

            elif op_type == "replace_text":
                old_text = op.get("old", "")
                new_text = op.get("new", "")
                for slide in prs.slides:
                    for shape in slide.shapes:
                        if shape.has_text_frame:
                            for para in shape.text_frame.paragraphs:
                                for run in para.runs:
                                    if old_text in run.text:
                                        run.text = run.text.replace(old_text, new_text)
                                        modified_count += 1

        prs.save(path)
        self.logger.info("modify: PPT 文档修改完成, path=%s, 修改数=%d", path, modified_count)
        return {
            "path": path,
            "modified_count": modified_count,
            "message": f"已执行 {modified_count} 项修改",
        }

    def convert(self, params: dict) -> dict:
        """格式转换"""
        self.logger.error("convert: PPT 格式转换暂未实现")
        return {"error": "PPT 格式转换暂未实现"}

    def analyze(self, params: dict) -> dict:
        """分析 PPT 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 PPT 文档, path=%s", path)

        prs = Presentation(path)
        self.logger.info("analyze: PPT 文档分析完成, path=%s, 幻灯片数=%d", path, len(prs.slides))
        return {
            "file_size": os.path.getsize(path),
            "slide_count": len(prs.slides),
            "width": prs.slide_width,
            "height": prs.slide_height,
        }
