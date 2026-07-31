import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../common/Icon";
import { PdfCanvasViewer } from "./PdfCanvasViewer";
import { MarkdownPreview } from "./MarkdownPreview";

interface PreviewOverlayProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  content?: string;
  fileType?: string;
  // PDF 文件的 base64 编码数据，用于 pdfjs-dist 渲染
  pdfBase64Data?: string | null;
}

export function PreviewOverlay({
  open,
  onClose,
  title = "",
  content = "",
  fileType,
  pdfBase64Data = null,
}: PreviewOverlayProps) {
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 bg-black/30 z-[200] flex items-center justify-center animate-fade-in"
    >
      <div
        className="w-4/5 max-w-[960px] h-[85vh] bg-bg rounded-[var(--radius-lg)] shadow-lg flex flex-col overflow-hidden animate-slide-up"
      >
        {/* 顶部栏 */}
        <div className="flex items-center px-5 py-3 border-b border-border gap-3 flex-shrink-0">
          <span className="font-semibold text-[14px] flex-1 truncate">{title}</span>
          <div className="flex gap-[6px] items-center">
            <button
              className="w-[30px] h-[30px] flex items-center justify-center rounded-[var(--radius-sm)] transition-colors text-text-secondary hover:bg-bg-sub"
              onClick={onClose}
            >
              <Icon name="close" size={18} />
            </button>
          </div>
        </div>

        {/* 内容区 */}
        {fileType?.toLowerCase() === "pdf" && pdfBase64Data ? (
          // PDF 真实渲染模式：PdfCanvasViewer 自带滚动和工具栏，不需要外层滚动包裹
          // 必须设置 flex flex-col，否则 PdfCanvasViewer 的 flex-1 不生效，导致高度为0
          <div className="flex-1 overflow-hidden flex flex-col">
            <ContentRenderer content={content} fileType={fileType} pdfBase64Data={pdfBase64Data} />
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto">
            <ContentRenderer content={content} fileType={fileType} pdfBase64Data={pdfBase64Data} />
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * 根据 fileType 选择对应的渲染方式
 */
function ContentRenderer({ content, fileType, pdfBase64Data }: { content: string; fileType?: string; pdfBase64Data?: string | null }) {
  const { t } = useTranslation();
  const normalizedType = fileType?.toLowerCase()?.trim() ?? "";

  // PDF 真实渲染预览：使用 pdfjs-dist Canvas 渲染
  if (normalizedType === "pdf" && pdfBase64Data) {
    return <PdfCanvasViewer base64Data={pdfBase64Data} />;
  }

  // Markdown 渲染
  if (normalizedType === "md" || normalizedType === "markdown") {
    return (
      <div className="px-10 py-8">
        <MarkdownPreview content={content} />
      </div>
    );
  }

  // Excel 表格渲染
  if (normalizedType === "xlsx") {
    return <ExcelTableRenderer content={content} />;
  }

  // Word / PPT / PDF 结构化渲染（PDF 无 base64 数据时降级为文本预览）
  if (normalizedType === "docx" || normalizedType === "pptx" || normalizedType === "pdf") {
    return <DocumentStructureRenderer content={content} fileType={normalizedType} />;
  }

  // 其他格式：纯文本显示
  return (
    <div className="px-10 py-8 leading-[1.8] text-text-secondary text-[14px] whitespace-pre-wrap">
      {content || (
        <div className="flex items-center justify-center h-full text-text-tertiary">
          {t("preview.noContent")}
        </div>
      )}
    </div>
  );
}

/**
 * Excel 表格渲染组件
 * 解析 JSON 格式的 Excel 数据并渲染为 HTML 表格
 * 数据格式: { sheets: { Sheet1: { data: [[...], [...]], row_count: N, col_count: M } }, sheet_names: ["Sheet1"] }
 */
function ExcelTableRenderer({ content }: { content: string }) {
  const { t } = useTranslation();
  // 尝试解析 JSON 数据
  const parsed = useMemo(() => {
    if (!content) return null;
    try {
      const data = JSON.parse(content);
      // 校验是否为 Excel 数据格式
      if (data && typeof data === "object" && data.sheets && typeof data.sheets === "object") {
        return data;
      }
      return null;
    } catch {
      return null;
    }
  }, [content]);

  // 解析失败时回退到纯文本显示
  if (!parsed) {
    return (
      <div className="px-10 py-8 leading-[1.8] text-text-secondary text-[14px] whitespace-pre-wrap">
        {content || (
          <div className="flex items-center justify-center h-full text-text-tertiary">
            {t("preview.noContent")}
          </div>
        )}
      </div>
    );
  }

  const sheetNames: string[] = parsed.sheet_names ?? Object.keys(parsed.sheets);

  return (
    <div className="px-6 py-6">
      {/* 工作表标签栏 */}
      {sheetNames.length > 1 && (
        <div className="flex gap-1 mb-4 border-b border-border pb-0">
          {sheetNames.map((name) => (
            <span
              key={name}
              className="px-3 py-1.5 text-[12px] font-medium text-text-secondary bg-bg-sub rounded-t-[var(--radius-sm)] border border-border border-b-0 -mb-px"
            >
              {name}
            </span>
          ))}
        </div>
      )}

      {/* 逐工作表渲染表格 */}
      {sheetNames.map((sheetName) => {
        const sheet = parsed.sheets[sheetName];
        if (!sheet || !Array.isArray(sheet.data)) return null;

        const rows: string[][] = sheet.data;
        if (rows.length === 0) return null;

        // 第一行作为表头
        const headerRow = rows[0];
        const bodyRows = rows.slice(1);

        return (
          <div key={sheetName} className="mb-6">
            {/* 多工作表时显示工作表名称 */}
            {sheetNames.length > 1 && (
              <div className="text-[13px] font-semibold text-text-primary mb-2">
                {sheetName}
                <span className="ml-2 text-[11px] font-normal text-text-tertiary">
                  {t("preview.rowXCol", { rows: sheet.row_count ?? bodyRows.length, cols: sheet.col_count ?? headerRow.length })}
                </span>
              </div>
            )}
            <div className="overflow-x-auto border border-border rounded-[var(--radius-sm)]">
              <table className="w-full border-collapse text-[13px]">
                <thead>
                  <tr className="bg-bg-sub">
                    {headerRow.map((cell, colIdx) => (
                      <th
                        key={colIdx}
                        className="px-3 py-2 text-left font-semibold text-text-primary border-b border-border whitespace-nowrap"
                      >
                        {cell ?? ""}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {bodyRows.map((row, rowIdx) => (
                    <tr
                      key={rowIdx}
                      className={rowIdx % 2 === 1 ? "bg-bg-sub/50" : ""}
                    >
                      {headerRow.map((_, colIdx) => (
                        <td
                          key={colIdx}
                          className="px-3 py-2 text-text-secondary border-b border-border-light whitespace-nowrap"
                        >
                          {row[colIdx] ?? ""}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/**
 * 结构化文档渲染组件
 * 用于 Word / PPT / PDF 的 JSON 格式化预览
 */
function DocumentStructureRenderer({ content, fileType }: { content: string; fileType: string }) {
  const { t } = useTranslation();
  // 尝试解析 JSON 数据
  const parsed = useMemo(() => {
    if (!content) return null;
    try {
      const data = JSON.parse(content);
      if (data && typeof data === "object") {
        return data;
      }
      return null;
    } catch {
      return null;
    }
  }, [content]);

  // 解析失败时回退到纯文本显示
  if (!parsed) {
    return (
      <div className="px-10 py-8 leading-[1.8] text-text-secondary text-[14px] whitespace-pre-wrap">
        {content || (
          <div className="flex items-center justify-center h-full text-text-tertiary">
            {t("preview.noContent")}
          </div>
        )}
      </div>
    );
  }

  if (fileType === "docx") {
    return <WordDocumentView data={parsed} />;
  }

  if (fileType === "pptx") {
    return <PptDocumentView data={parsed} />;
  }

  if (fileType === "pdf") {
    return <PdfDocumentView data={parsed} />;
  }

  // 未知结构化格式，回退纯文本
  return (
    <div className="px-10 py-8 leading-[1.8] text-text-secondary text-[14px] whitespace-pre-wrap">
      {content}
    </div>
  );
}

/**
 * Word 文档结构化视图
 * 数据格式: { paragraphs: [{text, style}], tables: [[[...]]], properties: {...} }
 */
function WordDocumentView({ data }: { data: Record<string, unknown> }) {
  const { t } = useTranslation();
  const paragraphs = Array.isArray(data.paragraphs) ? data.paragraphs : [];
  const tables = Array.isArray(data.tables) ? data.tables : [];
  const properties = data.properties && typeof data.properties === "object" ? data.properties as Record<string, unknown> : null;

  return (
    <div className="px-10 py-8">
      {/* 文档属性 */}
      {properties && Object.keys(properties).length > 0 && (
        <div className="mb-6 p-4 bg-bg-sub rounded-[var(--radius-sm)] border border-border-light">
          <div className="text-[12px] font-semibold text-text-primary mb-2">{t("preview.documentProperties")}</div>
          <div className="grid grid-cols-2 gap-x-6 gap-y-1 text-[12px]">
            {Object.entries(properties).map(([key, value]) => (
              <div key={key}>
                <span className="text-text-tertiary">{key}: </span>
                <span className="text-text-secondary">{String(value ?? "")}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 段落内容 */}
      {paragraphs.length > 0 && (
        <div className="space-y-1">
          {paragraphs.map((p: Record<string, unknown>, idx: number) => {
            const text = String(p.text ?? "");
            const style = String(p.style ?? "").toLowerCase();

            // 根据样式渲染标题层级
            if (style.includes("heading 1") || style.includes("title")) {
              return (
                <h1 key={idx} className="text-[20px] font-bold text-text-primary mt-6 mb-2">
                  {text}
                </h1>
              );
            }
            if (style.includes("heading 2")) {
              return (
                <h2 key={idx} className="text-[17px] font-bold text-text-primary mt-5 mb-1.5">
                  {text}
                </h2>
              );
            }
            if (style.includes("heading 3")) {
              return (
                <h3 key={idx} className="text-[15px] font-semibold text-text-primary mt-4 mb-1">
                  {text}
                </h3>
              );
            }
            if (style.includes("heading")) {
              return (
                <h4 key={idx} className="text-[14px] font-semibold text-text-primary mt-3 mb-1">
                  {text}
                </h4>
              );
            }

            // 空段落保留间距
            if (!text.trim()) {
              return <div key={idx} className="h-3" />;
            }

            // 普通段落
            return (
              <p key={idx} className="text-[14px] text-text-secondary leading-[1.8]">
                {text}
              </p>
            );
          })}
        </div>
      )}

      {/* 表格内容 */}
      {tables.length > 0 && (
        <div className="mt-6 space-y-4">
          {tables.map((table: unknown[][], tableIdx: number) => {
            if (!Array.isArray(table) || table.length === 0) return null;
            // 第一行作为表头
            const headerRow = table[0];
            const bodyRows = table.slice(1);
            return (
              <div key={tableIdx} className="overflow-x-auto border border-border rounded-[var(--radius-sm)]">
                <table className="w-full border-collapse text-[13px]">
                  <thead>
                    <tr className="bg-bg-sub">
                      {headerRow.map((cell: unknown, colIdx: number) => (
                        <th key={colIdx} className="px-3 py-2 text-left font-semibold text-text-primary border-b border-border whitespace-nowrap">
                          {String(cell ?? "")}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {bodyRows.map((row: unknown[], rowIdx: number) => (
                      <tr key={rowIdx} className={rowIdx % 2 === 1 ? "bg-bg-sub/50" : ""}>
                        {headerRow.map((_: unknown, colIdx: number) => (
                          <td key={colIdx} className="px-3 py-2 text-text-secondary border-b border-border-light whitespace-nowrap">
                            {String(row[colIdx] ?? "")}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          })}
        </div>
      )}

      {/* 无内容提示 */}
      {paragraphs.length === 0 && tables.length === 0 && !properties && (
        <div className="flex items-center justify-center h-32 text-text-tertiary text-[14px]">
          {t("preview.emptyDocument")}
        </div>
      )}
    </div>
  );
}

/**
 * PPT 文档结构化视图
 * 数据格式: { slides: [{ shapes: [{ name, text }] }], slide_count: N }
 */
function PptDocumentView({ data }: { data: Record<string, unknown> }) {
  const { t } = useTranslation();
  const slides = Array.isArray(data.slides) ? data.slides : [];
  const slideCount = typeof data.slide_count === "number" ? data.slide_count : slides.length;

  return (
    <div className="px-10 py-8">
      {/* 幻灯片统计 */}
      <div className="text-[12px] text-text-tertiary mb-4">
        {t("preview.slideCount", { count: slideCount })}
      </div>

      {/* 逐幻灯片渲染 */}
      <div className="space-y-4">
        {slides.map((slide: Record<string, unknown>, slideIdx: number) => {
          const shapes = Array.isArray(slide.shapes) ? slide.shapes : [];
          return (
            <div
              key={slideIdx}
              className="border border-border rounded-[var(--radius-md)] overflow-hidden"
            >
              {/* 幻灯片标题栏 */}
              <div className="px-4 py-2 bg-bg-sub border-b border-border flex items-center gap-2">
                <span className="w-5 h-5 rounded-full bg-accent/10 text-accent text-[11px] font-semibold flex items-center justify-center">
                  {slideIdx + 1}
                </span>
                <span className="text-[12px] font-medium text-text-primary">
                  {t("preview.slideNumber", { number: slideIdx + 1 })}
                </span>
              </div>

              {/* 幻灯片内容 */}
              <div className="px-5 py-4 space-y-2">
                {shapes.length > 0 ? (
                  shapes.map((shape: Record<string, unknown>, shapeIdx: number) => {
                    const name = String(shape.name ?? "");
                    const text = String(shape.text ?? "");

                    // 无文本的形状跳过
                    if (!text.trim()) return null;

                    // 标题形状特殊样式
                    const isTitle = name.toLowerCase().includes("title");
                    return (
                      <div key={shapeIdx}>
                        {isTitle ? (
                          <div className="text-[16px] font-semibold text-text-primary">
                            {text}
                          </div>
                        ) : (
                          <div className="text-[14px] text-text-secondary leading-[1.8]">
                            {text}
                          </div>
                        )}
                      </div>
                    );
                  })
                ) : (
                  <div className="text-[12px] text-text-tertiary">{t("preview.emptySlide")}</div>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* 无内容提示 */}
      {slides.length === 0 && (
        <div className="flex items-center justify-center h-32 text-text-tertiary text-[14px]">
          {t("preview.emptyDocument")}
        </div>
      )}
    </div>
  );
}

/**
 * PDF 文档结构化视图
 * 数据格式: { pages: [{ page_number, text }], page_count: N }
 */
function PdfDocumentView({ data }: { data: Record<string, unknown> }) {
  const { t } = useTranslation();
  const pages = Array.isArray(data.pages) ? data.pages : [];
  const pageCount = typeof data.page_count === "number" ? data.page_count : pages.length;

  return (
    <div className="px-10 py-8">
      {/* 页面统计 */}
      <div className="text-[12px] text-text-tertiary mb-4">
        {t("preview.pageCount", { count: pageCount })}
      </div>

      {/* 逐页渲染 */}
      <div className="space-y-4">
        {pages.map((page: Record<string, unknown>, pageIdx: number) => {
          const pageNumber = typeof page.page_number === "number" ? page.page_number : pageIdx + 1;
          const text = String(page.text ?? "");

          return (
            <div
              key={pageIdx}
              className="border border-border rounded-[var(--radius-md)] overflow-hidden"
            >
              {/* 页码标题栏 */}
              <div className="px-4 py-2 bg-bg-sub border-b border-border flex items-center gap-2">
                <span className="w-5 h-5 rounded-full bg-purple/10 text-purple text-[11px] font-semibold flex items-center justify-center">
                  {pageNumber}
                </span>
                <span className="text-[12px] font-medium text-text-primary">
                  {t("preview.pageNumber", { number: pageNumber })}
                </span>
              </div>

              {/* 页面文本内容 */}
              <div className="px-5 py-4 text-[14px] text-text-secondary leading-[1.8] whitespace-pre-wrap">
                {text.trim() || (
                  <span className="text-text-tertiary">{t("preview.emptyPage")}</span>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* 无内容提示 */}
      {pages.length === 0 && (
        <div className="flex items-center justify-center h-32 text-text-tertiary text-[14px]">
          {t("preview.emptyDocument")}
        </div>
      )}
    </div>
  );
}
