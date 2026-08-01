import { useMemo, useState, useCallback, useEffect, useRef, isValidElement, type ReactNode, type HTMLAttributes } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { Icon } from "../common/Icon";
import { PdfCanvasViewer } from "./PdfCanvasViewer";
import { MarkdownPreview } from "./MarkdownPreview";

interface PreviewPageProps {
  /** 预览文件名（作为页面标题） */
  title?: string;
  /** 预览文本内容 */
  content?: string;
  /** 预览文件类型 */
  fileType?: string;
  /** PDF 文件的 base64 编码数据，用于 pdfjs-dist 渲染 */
  pdfBase64Data?: string | null;
  /** 图片文件的 asset 协议 URL（convertFileSrc 转换），用于图片预览渲染 */
  imageSrc?: string | null;
  /** 预览文件所在目录的绝对路径，用于 Markdown 相对路径图片的解析 */
  baseDir?: string;
  /** 点击 Markdown 相对路径文件链接时的回调 */
  onOpenLink?: (href: string) => void;
  /** 内容加载中状态 */
  loading?: boolean;
  /** 返回回调（关闭预览，恢复工作流视图） */
  onBack: () => void;
}

/**
 * 文档预览页面
 * 替换主内容区（新建会话/工作流页面）显示文档预览内容
 * - 顶部栏：返回按钮 + 文件名标题
 * - 主体：按文件类型分派渲染（PDF / Markdown / Excel / Word / PPT / 源码文件）
 */
export function PreviewPage({
  title = "",
  content = "",
  fileType,
  pdfBase64Data = null,
  imageSrc = null,
  baseDir,
  onOpenLink,
  loading = false,
  onBack,
}: PreviewPageProps) {
  const { t } = useTranslation();
  // PDF 真实渲染模式：PdfCanvasViewer 自带滚动和工具栏，不需要外层滚动包裹
  const isPdfMode = fileType?.toLowerCase() === "pdf" && !!pdfBase64Data;

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* 顶部栏：返回按钮 + 标题 */}
      <div className="flex items-center gap-3 px-4 py-2 border-b border-border flex-shrink-0 h-11">
        <button
          className="flex items-center gap-1 px-2 py-1 bg-transparent border-none text-text-secondary cursor-pointer text-[13px] rounded transition-colors hover:bg-bg-sub hover:text-text-primary"
          onClick={onBack}
          title={t("preview.back")}
        >
          <Icon name="back" size={16} />
          <span>{t("preview.back")}</span>
        </button>
        <h3 className="text-[14px] font-medium text-text-primary m-0 truncate">{title}</h3>
      </div>

      {/* 主体内容区 */}
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {loading ? (
          <div className="flex items-center justify-center gap-2 flex-1 text-text-tertiary text-[13px]">
            <svg className="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            <span>{t("preview.loading")}</span>
          </div>
        ) : (
          // PDF 模式容器必须设置 flex flex-col，否则 PdfCanvasViewer 的 flex-1 不生效，导致高度为0
          <div className={isPdfMode ? "flex-1 overflow-hidden flex flex-col" : "flex-1 overflow-y-auto"}>
            <ContentRenderer content={content} fileType={fileType} pdfBase64Data={pdfBase64Data} imageSrc={imageSrc} title={title} baseDir={baseDir} onOpenLink={onOpenLink} />
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * 根据 fileType 选择对应的渲染方式
 */
function ContentRenderer({ content, fileType, pdfBase64Data, imageSrc, title, baseDir, onOpenLink }: { content: string; fileType?: string; pdfBase64Data?: string | null; imageSrc?: string | null; title?: string; baseDir?: string; onOpenLink?: (href: string) => void }) {
  const normalizedType = fileType?.toLowerCase()?.trim() ?? "";

  // 图片预览：asset 协议 URL 直接渲染（默认 100% 显示，工具栏与 Ctrl+滚轮可缩放）
  if (imageSrc) {
    return <ImagePreview imageSrc={imageSrc} title={title} />;
  }

  // PDF 真实渲染预览：使用 pdfjs-dist Canvas 渲染
  if (normalizedType === "pdf" && pdfBase64Data) {
    return <PdfCanvasViewer base64Data={pdfBase64Data} />;
  }

  // Markdown 渲染
  if (normalizedType === "md" || normalizedType === "markdown") {
    return (
      <div className="px-10 py-8">
        <MarkdownPreview content={content} baseDir={baseDir} onOpenLink={onOpenLink} />
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

  // 其他格式：源码/文本文件渲染为带行号的只读代码视图
  return <CodePreview content={content} fileType={normalizedType} />;
}

// 图片预览缩放参数
// 显示百分比以「原图 20%」为 100% 基准：新 100% = 原图 20%，默认显示 100%
const IMAGE_ZOOM_BASE = 0.2;
const IMAGE_ZOOM_STEP = 10;
const IMAGE_ZOOM_MIN = 50; // 显示 50% = 原图 10%
const IMAGE_ZOOM_MAX = 2000; // 显示 2000% = 原图 400%
const IMAGE_ZOOM_DEFAULT = 100;

/**
 * 图片预览组件
 * 默认以原图 20% 尺寸显示（显示为 100%），支持工具栏按钮与 Ctrl+鼠标滚轮缩放，
 * 点击百分比重置为默认
 */
function ImagePreview({ imageSrc, title }: { imageSrc: string; title?: string }) {
  const { t } = useTranslation();
  const [zoom, setZoom] = useState(IMAGE_ZOOM_DEFAULT);
  // 图片原始宽度（px），null 表示尚未加载就绪（就绪前不渲染 img，避免以原始尺寸瞬间闪出）
  const [naturalWidth, setNaturalWidth] = useState<number | null>(null);
  // 图片内容区容器引用（原生 wheel 监听用）
  const contentRef = useRef<HTMLDivElement>(null);

  // 实际渲染比例：显示百分比 × 基准（新 100% = 原图 20%）
  const renderScale = (zoom / 100) * IMAGE_ZOOM_BASE;

  // 预加载图片获取原始尺寸：img 挂载时即可带正确缩放宽度，消除加载瞬间的放大图闪烁
  useEffect(() => {
    let cancelled = false;
    setNaturalWidth(null);
    const probe = new Image();
    probe.onload = () => {
      if (!cancelled) setNaturalWidth(probe.naturalWidth);
    };
    probe.onerror = () => {
      // 加载失败（如损坏文件）：置 0 退出加载态，img 以原始尺寸显示
      if (!cancelled) setNaturalWidth(0);
    };
    probe.src = imageSrc;
    return () => {
      cancelled = true;
    };
  }, [imageSrc]);

  const zoomBy = useCallback((delta: number) => {
    setZoom((z) => Math.min(IMAGE_ZOOM_MAX, Math.max(IMAGE_ZOOM_MIN, z + delta)));
  }, []);

  const handleZoomIn = useCallback(() => zoomBy(IMAGE_ZOOM_STEP), [zoomBy]);

  const handleZoomOut = useCallback(() => zoomBy(-IMAGE_ZOOM_STEP), [zoomBy]);

  const handleReset = useCallback(() => setZoom(IMAGE_ZOOM_DEFAULT), []);

  // Ctrl + 鼠标滚轮缩放：原生监听（passive: false）确保能 preventDefault 阻止页面缩放
  useEffect(() => {
    const el = contentRef.current;
    if (!el) return;

    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey || e.deltaY === 0) return;
      e.preventDefault();
      zoomBy(e.deltaY < 0 ? IMAGE_ZOOM_STEP : -IMAGE_ZOOM_STEP);
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [zoomBy]);

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* 缩放工具栏 */}
      <div className="flex items-center gap-2 px-5 py-2 border-b border-border bg-bg-sub flex-shrink-0">
        <button
          className="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-text-secondary hover:bg-bg-hover transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          onClick={handleZoomOut}
          disabled={zoom <= IMAGE_ZOOM_MIN}
          title={t("preview.zoomOut")}
        >
          <Icon name="minus" size={14} />
        </button>
        <button
          className="text-[12px] text-text-secondary tabular-nums min-w-[40px] text-center hover:text-text-primary transition-colors cursor-pointer"
          onClick={handleReset}
          title={t("preview.zoomReset")}
        >
          {zoom}%
        </button>
        <button
          className="w-7 h-7 flex items-center justify-center rounded-[var(--radius-sm)] text-text-secondary hover:bg-bg-hover transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          onClick={handleZoomIn}
          disabled={zoom >= IMAGE_ZOOM_MAX}
          title={t("preview.zoomIn")}
        >
          <Icon name="plus" size={14} />
        </button>
      </div>

      {/* 图片内容区：flex 居中，超出容器尺寸时由 m-auto 保证滚动定位正确 */}
      <div ref={contentRef} className="flex-1 overflow-auto bg-bg-sub min-h-0 flex">
        {naturalWidth === null ? (
          // 图片加载占位：就绪前不渲染 img，避免以原始尺寸瞬间闪出放大图
          <div className="m-auto flex items-center gap-2 text-text-tertiary text-[13px]">
            <svg className="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            <span>{t("preview.loading")}</span>
          </div>
        ) : (
          <div className="m-auto p-8">
            <img
              src={imageSrc}
              alt={title ?? ""}
              style={{
                width:
                  naturalWidth > 0 ? Math.round(naturalWidth * renderScale) : undefined,
              }}
              className="h-auto"
            />
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * 源码文件预览组件
 * 将源码包装为 Markdown 围栏代码块，通过 react-markdown + rehype-highlight
 * 渲染为带行号的只读代码视图（类似 VS Code 预览模式，无边框/无复制按钮/不可编辑）
 */

// 扩展名 -> highlight.js 语言标识（对应 lowlight common 语言集）
const CODE_LANGUAGES: Record<string, string> = {
  // JavaScript / TypeScript 生态
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "javascript",
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "typescript",
  // Python / Rust / Go / Java / C# / C/C++
  py: "python",
  pyw: "python",
  pyi: "python",
  pyx: "python",
  ipynb: "json",
  rs: "rust",
  go: "go",
  java: "java",
  cs: "csharp",
  csx: "csharp",
  c: "c",
  h: "c",
  cc: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  hxx: "cpp",
  hh: "cpp",
  ino: "cpp",
  // 脚本与 Shell
  php: "php",
  phtml: "php",
  rb: "ruby",
  rake: "ruby",
  gemspec: "ruby",
  swift: "swift",
  kt: "kotlin",
  kts: "kotlin",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  fish: "bash",
  ksh: "bash",
  ps1: "powershell",
  psd1: "powershell",
  psm1: "powershell",
  bat: "dos",
  cmd: "dos",
  lua: "lua",
  r: "r",
  rmd: "r",
  pl: "perl",
  pm: "perl",
  // Web 前端
  html: "xml",
  htm: "xml",
  css: "css",
  scss: "scss",
  sass: "scss",
  less: "less",
  xml: "xml",
  // 数据与配置
  json: "json",
  jsonc: "json",
  json5: "json",
  yaml: "yaml",
  yml: "yaml",
  sql: "sql",
  txt: "plaintext",
  text: "plaintext",
  log: "plaintext",
};

/** 计算包裹源码的 Markdown 围栏长度，避免与源码中的反引号串冲突 */
function buildFence(content: string): string {
  let maxRun = 2;
  const matches = content.match(/`+/g);
  if (matches) {
    for (const m of matches) {
      if (m.length > maxRun) maxRun = m.length;
    }
  }
  return "`".repeat(maxRun + 1);
}

// react-markdown 传递给自定义组件的额外属性
type MdExtraProps = { node?: unknown; siblingCount?: number };

/** 从高亮后的 React 节点树中提取纯文本（用于计算代码行数） */
function extractPlainText(node: ReactNode): string {
  if (node == null || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(extractPlainText).join("");
  if (isValidElement(node)) {
    return extractPlainText((node.props as { children?: ReactNode }).children);
  }
  return "";
}

/**
 * 源码预览代码块：左侧行号列 + 只读代码区
 * 不显示 Markdown 代码块的边框、标题栏与复制按钮
 */
function SourceCodeBlock({
  children,
  node: _node,
  siblingCount: _sc,
  ...rest
}: HTMLAttributes<HTMLPreElement> & MdExtraProps) {
  // 提取 code 子元素的纯文本，按行生成行号
  const childElements = Array.isArray(children) ? children : children ? [children] : [];
  const codeChild = childElements.find(isValidElement);
  const plainText = codeChild ? extractPlainText(codeChild) : "";
  const lines = plainText.split("\n");
  // 末尾换行产生的空行不显示行号
  if (lines.length > 0 && lines[lines.length - 1] === "") lines.pop();
  const lineNumbers = Array.from({ length: Math.max(lines.length, 1) }, (_, i) => i + 1);

  return (
    <div className="code-preview-block">
      <div className="code-preview-gutter" aria-hidden="true">
        {lineNumbers.map((n) => (
          <span key={n}>{n}</span>
        ))}
      </div>
      <pre {...rest} className="code-preview-content">
        {children}
      </pre>
    </div>
  );
}

function CodePreview({ content, fileType }: { content: string; fileType: string }) {
  // 已识别的扩展名使用对应的 hljs 语言标识，未识别时以扩展名本身作为标识（无高亮但保持代码样式）
  const language = CODE_LANGUAGES[fileType] ?? fileType;
  const fence = buildFence(content);
  const markdown = `${fence}${language}\n${content}\n${fence}`;
  return (
    <div className="code-preview">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
        components={{ pre: SourceCodeBlock }}
      >
        {markdown}
      </ReactMarkdown>
      <style>{codePreviewStyles}</style>
    </div>
  );
}

// 源码预览样式：行号列 + 只读代码（无边框、无复制按钮、自包含 hljs 高亮配色）
const codePreviewStyles = `
/* ===== 源码预览布局 ===== */
/* 容器不固定高度：短文件背景铺满视口（min-height），长文件随内容撑高，
   由外层页面级容器统一垂直滚动（行号与代码同步滚动） */
.code-preview {
  min-height: 100%;
  display: flex;
  flex-direction: column;
}

.code-preview-block {
  flex: 1;
  display: flex;
  align-items: stretch;
  /* 不能设置 min-height: 100%，否则会覆盖 flex 子项默认的 min-height: auto，
     导致容器高度被压缩为视口高度、内容溢出并产生双滚动条 */
  background: var(--color-bg-elevated);
}

/* 行号列 */
.code-preview-gutter {
  flex-shrink: 0;
  min-width: 3.5rem;
  padding: 14px 0.8rem;
  text-align: right;
  user-select: none;
  color: var(--color-text-tertiary);
  background: var(--color-bg-sub);
  border-right: 1px solid var(--color-border-light);
}

.code-preview-gutter span {
  display: block;
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.6;
}

/* 代码区：长行横向滚动时行号保持固定；
   overflow-y 必须用 clip，避免与 overflow-x: auto 联动产生内部垂直滚动条 */
.code-preview-content {
  flex: 1;
  min-width: 0;
  margin: 0 !important;
  padding: 14px 16px !important;
  overflow-x: auto;
  overflow-y: clip;
  font-family: var(--font-mono) !important;
  font-size: 13px !important;
  line-height: 1.6 !important;
  white-space: pre !important;
  tab-size: 4;
  color: var(--color-text-secondary) !important;
}

.code-preview-content code {
  font-family: inherit !important;
  font-size: inherit !important;
  line-height: inherit !important;
  background: none !important;
  padding: 0 !important;
  border-radius: 0 !important;
  color: inherit !important;
}

/* ===== highlight.js 语法高亮 - 浅色主题（默认，白底高对比度配色） ===== */
.code-preview-content .hljs-keyword,
.code-preview-content .hljs-selector-tag,
.code-preview-content .hljs-literal { color: #cf222e; }

.code-preview-content .hljs-string,
.code-preview-content .hljs-doctag,
.code-preview-content .hljs-template-tag,
.code-preview-content .hljs-template-variable { color: #0a3069; }

.code-preview-content .hljs-number { color: #0550ae; }

.code-preview-content .hljs-built_in { color: #953800; }

.code-preview-content .hljs-comment,
.code-preview-content .hljs-quote { color: #57606a; font-style: italic; }

.code-preview-content .hljs-function .hljs-title,
.code-preview-content .hljs-title.function_ { color: #8250df; }

.code-preview-content .hljs-class .hljs-title,
.code-preview-content .hljs-title.class_ { color: #0550ae; }

.code-preview-content .hljs-variable,
.code-preview-content .hljs-attr { color: #953800; }

.code-preview-content .hljs-type,
.code-preview-content .hljs-params { color: #0550ae; }

.code-preview-content .hljs-meta { color: #cf222e; }

.code-preview-content .hljs-tag { color: #116329; }

.code-preview-content .hljs-name { color: #116329; }

.code-preview-content .hljs-attribute { color: #0a3069; }

.code-preview-content .hljs-symbol,
.code-preview-content .hljs-bullet { color: #0550ae; }

.code-preview-content .hljs-addition { color: #116329; background: rgba(17, 99, 41, 0.1); }

.code-preview-content .hljs-deletion { color: #82071e; background: rgba(130, 7, 30, 0.1); }

.code-preview-content .hljs-emphasis { font-style: italic; }

.code-preview-content .hljs-strong { font-weight: 600; }

.code-preview-content .hljs-regexp { color: #0a3069; }

.code-preview-content .hljs-property { color: #0550ae; }

.code-preview-content .hljs-section { color: #953800; }

/* ===== highlight.js 语法高亮 - 深色主题 ===== */
.dark .code-preview-content .hljs-keyword,
.dark .code-preview-content .hljs-selector-tag,
.dark .code-preview-content .hljs-literal { color: #569cd6; }

.dark .code-preview-content .hljs-string,
.dark .code-preview-content .hljs-doctag,
.dark .code-preview-content .hljs-template-tag,
.dark .code-preview-content .hljs-template-variable { color: #ce9178; }

.dark .code-preview-content .hljs-number,
.dark .code-preview-content .hljs-built_in { color: #b5cea8; }

.dark .code-preview-content .hljs-comment,
.dark .code-preview-content .hljs-quote { color: #6a9955; font-style: italic; }

.dark .code-preview-content .hljs-function .hljs-title,
.dark .code-preview-content .hljs-title.function_ { color: #dcdcaa; }

.dark .code-preview-content .hljs-class .hljs-title,
.dark .code-preview-content .hljs-title.class_ { color: #4ec9b0; }

.dark .code-preview-content .hljs-variable,
.dark .code-preview-content .hljs-attr { color: #9cdcfe; }

.dark .code-preview-content .hljs-type,
.dark .code-preview-content .hljs-params { color: #4ec9b0; }

.dark .code-preview-content .hljs-meta { color: #569cd6; }

.dark .code-preview-content .hljs-tag { color: #569cd6; }

.dark .code-preview-content .hljs-name { color: #569cd6; }

.dark .code-preview-content .hljs-attribute { color: #9cdcfe; }

.dark .code-preview-content .hljs-symbol,
.dark .code-preview-content .hljs-bullet { color: #d7ba7d; }

.dark .code-preview-content .hljs-addition { color: #b5cea8; background: rgba(181, 206, 168, 0.1); }

.dark .code-preview-content .hljs-deletion { color: #ce9178; background: rgba(206, 145, 120, 0.1); }

.dark .code-preview-content .hljs-emphasis { font-style: italic; }

.dark .code-preview-content .hljs-strong { font-weight: 600; }

.dark .code-preview-content .hljs-regexp { color: #d16969; }

.dark .code-preview-content .hljs-property { color: #9cdcfe; }

.dark .code-preview-content .hljs-section { color: #4ec9b0; }
`;

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
