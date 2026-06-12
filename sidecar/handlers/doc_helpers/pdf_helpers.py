"""PDF 文档生成 Helper 函数
封装 reportlab 常用操作，内置专业配色方案
"""

import os

try:
    from reportlab.lib.pagesizes import A4, letter
    from reportlab.lib.units import cm
    from reportlab.platypus import SimpleDocTemplate
    HAS_REPORTLAB = True
except ImportError:
    HAS_REPORTLAB = False

# 专业配色方案
THEME = {
    "heading1_color": (0x1F / 255, 0x4E / 255, 0x79 / 255),  # 深蓝色
    "heading2_color": (0x2E / 255, 0x75 / 255, 0xB6 / 255),  # 中蓝色
    "heading3_color": (0x5B / 255, 0x9B / 255, 0xD5 / 255),  # 浅蓝色
    "table_header_bg": (0xD6 / 255, 0xE4 / 255, 0xF0 / 255),
    "table_alt_row_bg": (0xED / 255, 0xF2 / 255, 0xF9 / 255),
    "text_color": (0x33 / 255, 0x33 / 255, 0x33 / 255),
}


def create_pdf_doc(title=None, page_size="a4", author=""):
    """创建一个预配置好专业样式的 PDF 文档对象

    Args:
        title: 文档标题（可选）
        page_size: 页面尺寸 "a4" 或 "letter"
        author: 文档作者

    Returns:
        dict: 文档配置字典，需传给 save_pdf_doc()

    示例:
        config = create_pdf_doc(title="报告", author="张三")
        # 添加内容到 story...
        save_pdf_doc(story, "报告.pdf", doc_config=config)
    """
    if not HAS_REPORTLAB:
        raise ImportError("reportlab 未安装，无法生成 PDF")

    pagesize = letter if page_size == "letter" else A4

    return {
        "pagesize": pagesize,
        "title": title or "",
        "author": author or "",
        "leftMargin": 2.54 * cm,
        "rightMargin": 2.54 * cm,
        "topMargin": 2.54 * cm,
        "bottomMargin": 2.54 * cm,
    }


def save_pdf_doc(story, filename, working_dir="", doc_config=None):
    """保存 PDF 文档到工作目录

    Args:
        story: reportlab Story 对象（内容列表）
        filename: 文件名（如 "报告.pdf"）
        working_dir: 工作目录路径
        doc_config: 文档配置（来自 create_pdf_doc 的返回值）

    Returns:
        str: 保存的文件绝对路径
    """
    if not HAS_REPORTLAB:
        raise ImportError("reportlab 未安装，无法生成 PDF")

    output_path = os.path.join(working_dir, filename) if working_dir else filename
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)

    config = doc_config or {}
    doc = SimpleDocTemplate(
        output_path,
        pagesize=config.get("pagesize"),
        title=config.get("title", ""),
        author=config.get("author", ""),
        leftMargin=config.get("leftMargin", 2.54 * cm),
        rightMargin=config.get("rightMargin", 2.54 * cm),
        topMargin=config.get("topMargin", 2.54 * cm),
        bottomMargin=config.get("bottomMargin", 2.54 * cm),
    )
    doc.build(story)

    return output_path
