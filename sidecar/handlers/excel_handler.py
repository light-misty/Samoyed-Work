"""Excel 文档处理器
基于 openpyxl 实现 Excel 文档的生成、读取、修改
"""

import os
import logging
from typing import Any

from openpyxl import Workbook, load_workbook
from openpyxl.styles import Font, Alignment, Border, Side, PatternFill
from openpyxl.utils import get_column_letter


class ExcelHandler:
    """Excel (.xlsx) 文档处理器"""

    logger = logging.getLogger(__name__)

    def generate(self, params: dict) -> dict:
        """生成 Excel 文档

        params:
            path: 输出文件路径
            sheets: 工作表列表
                [{"name": "Sheet1", "data": [[...]], "headers": [...]}]
        """
        path = params.get("path", "")
        sheets = params.get("sheets", [])
        if not path:
            self.logger.error("generate: 缺少输出文件路径")
            return {"error": "缺少输出文件路径"}

        self.logger.info("generate: 开始生成 Excel 文档, path=%s, 工作表数=%d", path, len(sheets))

        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)

        wb = Workbook()
        # 删除默认工作表
        default_sheet = wb.active
        if default_sheet:
            wb.remove(default_sheet)

        for sheet_info in sheets:
            sheet_name = sheet_info.get("name", "Sheet1")
            data = sheet_info.get("data", [])
            headers = sheet_info.get("headers", [])

            ws = wb.create_sheet(title=sheet_name)

            # 写入表头
            if headers:
                for col_idx, header in enumerate(headers, 1):
                    cell = ws.cell(row=1, column=col_idx, value=header)
                    cell.font = Font(bold=True)
                    cell.alignment = Alignment(horizontal="center")

            # 写入数据
            start_row = 2 if headers else 1
            for row_idx, row_data in enumerate(data, start_row):
                for col_idx, value in enumerate(row_data, 1):
                    ws.cell(row=row_idx, column=col_idx, value=value)

            # 自动调整列宽
            for col in ws.columns:
                max_length = 0
                col_letter = get_column_letter(col[0].column)
                for cell in col:
                    if cell.value:
                        max_length = max(max_length, len(str(cell.value)))
                ws.column_dimensions[col_letter].width = min(max_length + 2, 50)

        wb.save(path)
        self.logger.info("generate: Excel 文档已生成, path=%s, 工作表数=%d", path, len(sheets))
        return {
            "path": path,
            "sheet_count": len(sheets),
            "message": f"Excel 文档已生成: {path}",
        }

    def read(self, params: dict) -> dict:
        """读取 Excel 文档

        params:
            path: 文件路径
            sheet: 工作表名称（可选，默认读取所有）
            range: 读取范围（可选，如 "A1:D10"）
        """
        path = params.get("path", "")
        sheet_name = params.get("sheet", None)
        read_range = params.get("range", None)
        if not path:
            self.logger.error("read: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("read: 开始读取 Excel 文档, path=%s", path)

        wb = load_workbook(path, data_only=True)
        result = {"sheets": {}}

        if sheet_name:
            sheets_to_read = [sheet_name]
        else:
            sheets_to_read = wb.sheetnames

        for name in sheets_to_read:
            if name not in wb.sheetnames:
                continue
            ws = wb[name]

            if read_range:
                rows = []
                for row in ws[read_range]:
                    rows.append([cell.value for cell in row])
            else:
                rows = []
                for row in ws.iter_rows(values_only=True):
                    rows.append(list(row))

            result["sheets"][name] = {
                "data": rows,
                "row_count": ws.max_row,
                "col_count": ws.max_column,
            }

        result["sheet_names"] = wb.sheetnames
        self.logger.info("read: Excel 文档读取完成, path=%s, 工作表数=%d", path, len(result["sheets"]))
        return result

    def modify(self, params: dict) -> dict:
        """修改 Excel 文档

        params:
            path: 文件路径
            operations: 修改操作列表
        """
        path = params.get("path", "")
        operations = params.get("operations", [])
        if not path:
            self.logger.error("modify: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("modify: 开始修改 Excel 文档, path=%s, 操作数=%d", path, len(operations))

        wb = load_workbook(path)
        modified_count = 0

        for op in operations:
            op_type = op.get("type", "")

            if op_type == "set_cell":
                sheet = op.get("sheet", wb.active.title if wb.active else "Sheet1")
                if sheet in wb.sheetnames:
                    ws = wb[sheet]
                    row = op.get("row", 1)
                    col = op.get("col", 1)
                    value = op.get("value", "")
                    ws.cell(row=row, column=col, value=value)
                    modified_count += 1

            elif op_type == "add_sheet":
                name = op.get("name", f"Sheet{len(wb.sheetnames) + 1}")
                if name not in wb.sheetnames:
                    wb.create_sheet(title=name)
                    modified_count += 1

            elif op_type == "delete_sheet":
                name = op.get("name", "")
                if name in wb.sheetnames and len(wb.sheetnames) > 1:
                    wb.remove(wb[name])
                    modified_count += 1

            elif op_type == "set_range":
                sheet = op.get("sheet", wb.active.title if wb.active else "Sheet1")
                if sheet in wb.sheetnames:
                    ws = wb[sheet]
                else:
                    ws = wb.create_sheet(title=sheet)
                start_row = op.get("start_row", 1)
                start_col = op.get("start_col", 1)
                data = op.get("data", [])
                for i, row_data in enumerate(data):
                    for j, value in enumerate(row_data):
                        ws.cell(row=start_row + i, column=start_col + j, value=value)
                    modified_count += 1

        wb.save(path)
        self.logger.info("modify: Excel 文档修改完成, path=%s, 修改数=%d", path, modified_count)
        return {
            "path": path,
            "modified_count": modified_count,
            "message": f"已执行 {modified_count} 项修改",
        }

    def convert(self, params: dict) -> dict:
        """格式转换"""
        self.logger.error("convert: Excel 格式转换暂未实现")
        return {"error": "Excel 格式转换暂未实现"}

    def analyze(self, params: dict) -> dict:
        """分析 Excel 文档"""
        path = params.get("path", "")
        if not path:
            self.logger.error("analyze: 缺少文件路径")
            return {"error": "缺少文件路径"}
        if not os.path.exists(path):
            raise FileNotFoundError(path)

        self.logger.info("analyze: 开始分析 Excel 文档, path=%s", path)

        wb = load_workbook(path, data_only=True)
        sheets_info = []
        for name in wb.sheetnames:
            ws = wb[name]
            sheets_info.append({
                "name": name,
                "rows": ws.max_row,
                "cols": ws.max_column,
            })

        self.logger.info("analyze: Excel 文档分析完成, path=%s, 工作表数=%d", path, len(wb.sheetnames))
        return {
            "file_size": os.path.getsize(path),
            "sheet_count": len(wb.sheetnames),
            "sheets": sheets_info,
        }
