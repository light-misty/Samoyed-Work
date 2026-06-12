"""图表生成 Helper 函数
封装 matplotlib 常用图表，自动保存为图片文件
"""

import os

try:
    import matplotlib
    matplotlib.use('Agg')  # 无头模式，不弹出窗口
    import matplotlib.pyplot as plt
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False


def create_chart(chart_type="bar", data=None, title="", xlabel="", ylabel="",
                 filename="chart.png", working_dir="", **kwargs):
    """创建图表并保存为图片文件

    Args:
        chart_type: 图表类型 "bar"|"line"|"pie"|"scatter"|"area"|"hist"
        data: 图表数据（dict 或 DataFrame）
        title: 图表标题
        xlabel: X 轴标签
        ylabel: Y 轴标签
        filename: 输出文件名
        working_dir: 工作目录

    Returns:
        str: 图片文件绝对路径

    示例:
        chart_path = create_chart(
            chart_type="bar",
            data={"x": ["Q1", "Q2", "Q3", "Q4"], "y": [100, 150, 120, 180]},
            title="季度销售额",
            ylabel="万元",
            filename="sales_chart.png"
        )
    """
    if not HAS_MATPLOTLIB:
        raise ImportError("matplotlib 未安装，无法生成图表")
    fig, ax = plt.subplots(figsize=kwargs.get("figsize", (8, 5)))

    # 设置中文字体
    plt.rcParams['font.sans-serif'] = ['Microsoft YaHei', 'SimHei', 'Arial']
    plt.rcParams['axes.unicode_minus'] = False

    if chart_type == "bar":
        ax.bar(data["x"], data["y"], color=kwargs.get("color", "#2E75B6"))
    elif chart_type == "line":
        ax.plot(data["x"], data["y"], marker='o', color=kwargs.get("color", "#2E75B6"))
    elif chart_type == "pie":
        ax.pie(data["values"], labels=data["labels"], autopct='%1.1f%%')
    elif chart_type == "scatter":
        ax.scatter(data["x"], data["y"], color=kwargs.get("color", "#2E75B6"))
    elif chart_type == "area":
        ax.fill_between(data["x"], data["y"], alpha=0.3, color=kwargs.get("color", "#2E75B6"))
        ax.plot(data["x"], data["y"], color=kwargs.get("color", "#2E75B6"))
    elif chart_type == "hist":
        ax.hist(data["values"], bins=kwargs.get("bins", 10), color=kwargs.get("color", "#2E75B6"))

    if title:
        ax.set_title(title, fontsize=14, fontweight='bold')
    if xlabel:
        ax.set_xlabel(xlabel)
    if ylabel:
        ax.set_ylabel(ylabel)

    plt.tight_layout()

    output_path = os.path.join(working_dir, filename) if working_dir else filename
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    fig.savefig(output_path, dpi=150, bbox_inches='tight')
    plt.close(fig)

    return output_path


def save_chart(fig, filename, working_dir=""):
    """保存 matplotlib 图表到工作目录

    Args:
        fig: matplotlib Figure 对象
        filename: 输出文件名
        working_dir: 工作目录

    Returns:
        str: 图片文件绝对路径
    """
    if not HAS_MATPLOTLIB:
        raise ImportError("matplotlib 未安装，无法保存图表")
    output_path = os.path.join(working_dir, filename) if working_dir else filename
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    fig.savefig(output_path, dpi=150, bbox_inches='tight')
    plt.close(fig)

    return output_path
