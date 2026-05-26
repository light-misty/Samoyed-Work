//! 文档设计指导模块
//! 整合自 .trae/skills/ 中的 docx/xlsx/pptx/pdf Skill 文件
//! 为 Agent System Prompt 提供专业的文档生成规范

/// Word 文档设计指导
pub const WORD_DESIGN_GUIDE: &str = r#"
## Word 文档生成规范

### 页面尺寸
- US Letter: 12240 x 15840 DXA（美国文档默认）
- A4: 11906 x 16838 DXA（国际文档默认）
- 1 inch = 1440 DXA
- python-docx 中设置: section.page_width, section.page_height

### 样式规范
- 默认字体: Arial 12pt
- 标题1: 16pt 粗体, 间距前后 240 DXA
- 标题2: 14pt 粗体, 间距前后 180 DXA
- 正文: 12pt, 行间距 1.5

### 表格规范
- 必须设置表格宽度（DXA 单位）
- 同时设置列宽和每个单元格的宽度
- python-docx 中使用 table.columns[i].width 和 cell.width 设置宽度
- 边框: 单线 1pt 灰色 (#CCCCCC)
- 使用 ShadingType.CLEAR 而非 SOLID 防止黑色背景

### 列表规范
- 使用 python-docx 的列表样式（WD_STYLE_PARAGRAPH.LIST_BULLET）而非 Unicode 字符
- 缩进: 左 720 DXA, 悬挂 360 DXA

### 图片规范
- 必须指定图片格式（png/jpg/jpeg/gif/bmp/svg）
- python-docx 中使用 document.add_picture() 插入图片, 需指定 width/height
- 提供 altText 三字段: title, description, name

### 页眉页脚
- 使用 section.header / section.footer API
- 支持页码: 插入 PageNumber 域代码

### 超链接
- 外部链接: 使用 python-docx 的 OxmlElement 创建 hyperlink
- 内部链接: 使用书签 + 超链接引用

### 颜色编码标准
- 蓝色文字 (RGB: 0,0,255): 硬编码输入值
- 黑色文字 (RGB: 0,0,0): 所有公式和计算
- 绿色文字 (RGB: 0,128,0): 跨工作表引用
- 红色文字 (RGB: 255,0,0): 外部文件链接
"#;

/// Excel 文档设计指导
pub const EXCEL_DESIGN_GUIDE: &str = r#"
## Excel 文档生成规范

### 核心原则：使用公式而非硬编码值
- 错误: 在 Python 中计算 sum, 然后硬编码结果
- 正确: 使用 Excel 公式 =SUM(B2:B9)
- 公式单元格写入方式: ws['B10'] = '=SUM(B2:B9)'

### 数字格式标准
- 年份: 格式化为文本字符串（"2024" 而非 "2,024"）
- 货币: 使用 $#,##0 格式, 标题中必须注明单位
- 零值: 使用数字格式将零显示为 "-"
- 百分比: 默认 0.0% 格式（一位小数）
- 倍数: 格式化为 0.0x
- 负数: 使用括号 (123) 而非减号 -123

### 颜色编码标准
- 蓝色文字 (RGB: 0,0,255): 硬编码输入值, 用户会修改的数字
- 黑色文字 (RGB: 0,0,0): 所有公式和计算
- 绿色文字 (RGB: 0,128,0): 跨工作表引用
- 红色文字 (RGB: 255,0,0): 外部文件链接
- 黄色背景 (RGB: 255,255,0): 关键假设

### 库选择指南
- pandas: 数据分析、批量操作、简单数据导出
- openpyxl: 复杂格式、公式、Excel 特定功能

### openpyxl 注意事项
- 单元格索引从 1 开始 (row=1, column=1 即 A1)
- data_only=True 读取计算值, 但保存会丢失公式
- 公式不会被 Python 计算, 需要 Excel 或 recalc 脚本
"#;

/// PPT 文档设计指导
pub const PPT_DESIGN_GUIDE: &str = r#"
## PPT 文档生成规范

### 设计原则
- 不要创建无聊的幻灯片
- 选择大胆的、内容驱动的颜色方案
- 一种颜色占主导（60-70% 视觉权重）
- 深色背景用于标题和结论页, 浅色用于内容页

### 颜色方案库
| 方案 | 主色 | 辅色 | 强调色 |
|------|------|------|--------|
| Midnight Executive | #1E2761 (navy) | #CADCFC (ice blue) | #FFFFFF (white) |
| Forest & Moss | #2C5F2D (forest) | #97BC62 (moss) | #F5F5F5 (cream) |
| Coral Energy | #F96167 (coral) | #F9E795 (gold) | #2F3C7E (navy) |
| Ocean Gradient | #065A82 (deep blue) | #1C7293 (teal) | #21295C (midnight) |
| Charcoal Minimal | #36454F (charcoal) | #F2F2F2 (off-white) | #212121 (black) |

### 字体规范
| 元素 | 大小 |
|------|------|
| 幻灯片标题 | 36-44pt 粗体 |
| 节标题 | 20-24pt 粗体 |
| 正文 | 14-16pt |
| 注释 | 10-12pt 淡色 |

### 间距规范
- 最小边距: 0.5 inch
- 内容块间距: 0.3-0.5 inch
- 留白呼吸空间, 不要填满每一寸

### 避免的错误
- 不要重复相同的布局
- 不要居中正文段落
- 不要默认使用蓝色
- 不要创建纯文字幻灯片
- 不要在标题下使用强调线
"#;

/// PDF 文档设计指导
pub const PDF_DESIGN_GUIDE: &str = r#"
## PDF 文档生成规范

### 下标和上标
- 不要使用 Unicode 下标/上标字符
- reportlab 中使用 XML 标签: H<sub>2</sub>O, x<super>2</super>
- 在 Paragraph 中使用: Paragraph("H<sub>2</sub>O", style)

### 高级操作
- 合并: 使用 pypdf 的 PdfWriter.add_page()
- 拆分: 每页单独保存
- 旋转: page.rotate(90)
- 水印: page.merge_page(watermark)
- 加密: writer.encrypt(user_pwd, owner_pwd)

### 表格提取
- 使用 pdfplumber 的 pdfplumber.extract_tables()

### reportlab 注意事项
- 使用 SimpleDocTemplate + Paragraph 创建结构化 PDF
- 特殊字符必须使用 html.escape() 转义
- 中文字体需注册后使用
"#;

/// 获取所有文档设计指导, 拼接为完整字符串
pub fn get_all_design_guides() -> String {
    format!(
        "{}\n\n{}\n\n{}\n\n{}",
        WORD_DESIGN_GUIDE,
        EXCEL_DESIGN_GUIDE,
        PPT_DESIGN_GUIDE,
        PDF_DESIGN_GUIDE,
    )
}

/// 根据文档类型获取对应的设计指导
pub fn get_design_guide_by_type(doc_type: &str) -> &'static str {
    match doc_type {
        "docx" => WORD_DESIGN_GUIDE,
        "xlsx" => EXCEL_DESIGN_GUIDE,
        "pptx" => PPT_DESIGN_GUIDE,
        "pdf" => PDF_DESIGN_GUIDE,
        _ => "",
    }
}
