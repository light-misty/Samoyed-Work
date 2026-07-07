"""PDF 中文字体注册工具

提供跨平台的 reportlab 中文字体注册功能，供各 handler 共享使用。
避免在多个 handler 中重复相同的字体搜索与注册逻辑。

主字体: 微软雅黑 (Microsoft YaHei)，Windows 系统自带
回退字体: 宋体、黑体、苹方、Noto Sans CJK 等
"""

import os
import logging

logger = logging.getLogger(__name__)

# 跨平台中文字体路径列表（按优先级排序）
_FONT_PATHS = [
    # Windows - 微软雅黑（主字体）
    ("MicrosoftYaHei", "C:/Windows/Fonts/msyh.ttc", 0),
    # Windows - 微软雅黑粗体
    ("MicrosoftYaHei-Bold", "C:/Windows/Fonts/msyhbd.ttc", 0),
    # Windows - 宋体
    ("ChineseFont", "C:/Windows/Fonts/simsun.ttc", 0),
    # Windows - 黑体
    ("ChineseFont", "C:/Windows/Fonts/simhei.ttf", 0),
    # macOS - 苹方
    ("ChineseFont", "/System/Library/Fonts/PingFang.ttc", 0),
    # macOS - 华文黑体
    ("ChineseFont", "/System/Library/Fonts/STHeiti Light.ttc", 0),
    # macOS - Arial Unicode
    ("ChineseFont", "/Library/Fonts/Arial Unicode.ttf", 0),
    # Linux - Noto Sans CJK
    ("ChineseFont", "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 0),
    ("ChineseFont", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    # Linux - 文泉驿
    ("ChineseFont", "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", 0),
    ("ChineseFont", "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
    # Linux - DroidSans
    ("ChineseFont", "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf", 0),
]


def register_chinese_font() -> str:
    """注册 reportlab 中文字体，返回可用的字体名称

    按优先级尝试注册系统中的中文字体，优先注册微软雅黑 (Microsoft YaHei)。
    若均不可用则回退到 Helvetica。

    Returns:
        str: 已注册的字体名称，微软雅黑可用时为 "MicrosoftYaHei"，其他中文字体为 "ChineseFont"，否则为 "Helvetica"

    Raises:
        ImportError: reportlab 未安装时抛出
    """
    from reportlab.pdfbase import pdfmetrics
    from reportlab.pdfbase.ttfonts import TTFont

    font_name = "Helvetica"
    for name, fp, subfont_idx in _FONT_PATHS:
        if os.path.exists(fp):
            try:
                # TTC 字体需要指定 subfontIndex
                pdfmetrics.registerFont(TTFont(name, fp, subfontIndex=subfont_idx))
                # 第一个成功注册的字体作为返回值
                if font_name == "Helvetica":
                    font_name = name
                logger.debug("register_chinese_font: 成功注册字体 %s (%s)", name, fp)
            except Exception as e:
                logger.debug("register_chinese_font: 注册字体失败 %s (%s): %s", name, fp, e)
                continue

    # 全部字体注册失败时，记录警告而非静默回退
    # 返回 Helvetica 会导致 PDF 中文显示为方块，调用方应检查返回值并决定是否中止
    if font_name == "Helvetica":
        logger.warning("register_chinese_font: 所有中文字体注册失败，回退到 Helvetica（中文将显示为方块）")
    return font_name


def register_bold_font() -> str:
    """注册 reportlab 中文粗体字体

    Returns:
        str: 已注册的粗体字体名称，若不可用则返回空字符串
    """
    from reportlab.pdfbase import pdfmetrics
    from reportlab.pdfbase.ttfonts import TTFont

    bold_path = "C:/Windows/Fonts/msyhbd.ttc"
    if os.path.exists(bold_path):
        try:
            pdfmetrics.registerFont(TTFont("MicrosoftYaHei-Bold", bold_path, subfontIndex=0))
            return "MicrosoftYaHei-Bold"
        except Exception:
            pass
    return ""


# ============================================================================
# PyMuPDF (fitz) 专用字体注册工具
#
# 适用场景：使用 fitz 修改现有 PDF 时注册中文字体
# 注意：与 reportlab 不同，fitz 不能用 fontname 直接引用系统字体名称，
#       必须通过 fontbuffer 传入 TTF/OTF 字节数据。TTC 需先 fitz.Font 提取子集。
# ============================================================================

# fitz 专用字体路径列表（按优先级排序）
_FITZ_FONT_PATHS = [
    # Windows - 微软雅黑粗体（主粗体字体）
    ("C:/Windows/Fonts/msyhbd.ttc", 0, True),
    # Windows - 微软雅黑常规
    ("C:/Windows/Fonts/msyh.ttc", 0, False),
    # Windows - 黑体（常规回退）
    ("C:/Windows/Fonts/simhei.ttf", 0, False),
    # Windows - 宋体（常规回退）
    ("C:/Windows/Fonts/simsun.ttc", 0, False),
    # macOS - 苹方
    ("/System/Library/Fonts/PingFang.ttc", 0, False),
    # Linux - Noto Sans CJK
    ("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 0, False),
]


def register_fitz_font(page, font_name="MyZhFont", bold=False, idx=0):
    """为 fitz.Page 注册中文字体（支持 TTC 格式）

    适用场景：使用 fitz 修改现有 PDF 时，给页面注册中文字体以便后续 insert_text 使用。
    内部通过 fitz.Font 加载 TTC 并提取子集字节数据（font.buffer），
    再通过 page.insert_font(fontbuffer=...) 注册，规避 TTC 直接传入失败的问题。

    关键约束：必须在 page.apply_redactions() 之后调用。
    PyMuPDF 的 apply_redactions() 会重置页面字体注册状态，
    因此注册顺序必须是：apply_redactions() -> register_fitz_font() -> insert_text()

    Args:
        page: fitz.Page 对象，待注册字体的页面
        font_name: 自定义字体注册名（不能含空格，否则 insert_text 会报错）
                   默认 "MyZhFont"，引用时用 page.insert_text(..., fontname=font_name)
        bold: 是否注册粗体字体，True 时优先使用微软雅黑粗体
        idx: TTC 字体的子字体索引（通常 0 即可）

    Returns:
        str: 注册成功时返回 font_name，失败返回空字符串

    Raises:
        ImportError: fitz (PyMuPDF) 未安装时抛出
    """
    import fitz

    # 按优先级尝试加载字体
    for font_path, subfont_idx, is_bold in _FITZ_FONT_PATHS:
        # 粗体模式跳过非粗体字体，常规模式跳过粗体字体
        if bold != is_bold:
            continue
        if not os.path.exists(font_path):
            continue
        try:
            # fitz.Font 加载 TTC 字体，提取子字体（idx 指定 TTC 中的字体索引）
            # font.buffer 是子集字体的 TTF 字节数据，可被 insert_font 接受
            font = fitz.Font(fontfile=font_path, idx=subfont_idx)
            # insert_font 注册到页面，fontname 必须无空格
            page.insert_font(fontname=font_name, fontbuffer=font.buffer)
            logger.debug("register_fitz_font: 成功注册字体 %s (%s)", font_name, font_path)
            return font_name
        except Exception as e:
            logger.debug("register_fitz_font: 注册字体失败 %s: %s", font_path, e)
            continue

    logger.warning("register_fitz_font: 所有中文字体注册失败")
    return ""


def create_fitz_font(bold=False):
    """创建 fitz.Font 对象（支持 TTC 格式）

    适用场景：需要独立使用 fitz.Font 对象时（如 TextWriter 场景）。
    与 register_fitz_font 不同，此函数返回 fitz.Font 对象而非注册名。

    Args:
        bold: 是否创建粗体字体

    Returns:
        fitz.Font: 字体对象，失败返回 None
    """
    import fitz

    for font_path, subfont_idx, is_bold in _FITZ_FONT_PATHS:
        if bold != is_bold:
            continue
        if not os.path.exists(font_path):
            continue
        try:
            font = fitz.Font(fontfile=font_path, idx=subfont_idx)
            logger.debug("create_fitz_font: 成功创建字体 %s", font_path)
            return font
        except Exception as e:
            logger.debug("create_fitz_font: 创建字体失败 %s: %s", font_path, e)
            continue

    logger.warning("create_fitz_font: 所有中文字体创建失败")
    return None
