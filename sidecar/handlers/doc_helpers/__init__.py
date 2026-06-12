"""DocAgent 文档生成 Helper 函数库
提供封装好的文档生成函数，内置专业配色方案和样式规范
"""

from .word_helpers import create_word_doc, save_word_doc
from .excel_helpers import create_excel_doc, save_excel_doc
from .ppt_helpers import create_ppt_doc, save_ppt_doc
from .pdf_helpers import create_pdf_doc, save_pdf_doc
from .chart_helpers import create_chart, save_chart
from .common import (
    THEME_COLORS,      # 专业配色方案
    apply_theme,       # 应用配色方案
    add_styled_table,  # 添加专业样式表格
)

__all__ = [
    'create_word_doc', 'save_word_doc',
    'create_excel_doc', 'save_excel_doc',
    'create_ppt_doc', 'save_ppt_doc',
    'create_pdf_doc', 'save_pdf_doc',
    'create_chart', 'save_chart',
    'THEME_COLORS', 'apply_theme', 'add_styled_table',
]
