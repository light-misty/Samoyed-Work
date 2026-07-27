/**
 * 斜杠命令定义接口
 */
export interface SlashCommand {
  /** 命令名（不含 / 前缀），如 "help"、"compact" */
  name: string;
  /** 命令描述（用于菜单显示，存储 i18n key，由组件使用 t() 函数翻译） */
  description: string;
  /** 完整用法示例，如 "/ws <工作区名称>" */
  usage: string;
  /** Agent 运行中是否允许执行 */
  allowedInAgent: boolean;
  /** 是否需要参数 */
  requiresArgs: boolean;
  /** 参数提示（用于参数缺失时的 Toast，存储 i18n key） */
  argHint?: string;
}

/**
 * 斜杠命令注册表
 * description 与 argHint 字段存储 i18n key，由组件使用 t() 函数翻译
 */
export const SLASH_COMMANDS: SlashCommand[] = [
  {
    name: "help",
    description: "slash.commands.help.desc",
    usage: "/help",
    allowedInAgent: true,
    requiresArgs: false,
  },
  {
    name: "compact",
    description: "slash.commands.compact.desc",
    usage: "/compact",
    allowedInAgent: false,
    requiresArgs: false,
  },
  {
    name: "retry",
    description: "slash.commands.retry.desc",
    usage: "/retry",
    allowedInAgent: false,
    requiresArgs: false,
  },
  {
    name: "stop",
    description: "slash.commands.stop.desc",
    usage: "/stop",
    allowedInAgent: true,
    requiresArgs: false,
  },
  {
    name: "new",
    description: "slash.commands.new.desc",
    usage: "/new",
    allowedInAgent: true,
    requiresArgs: false,
  },
  {
    name: "stats",
    description: "slash.commands.stats.desc",
    usage: "/stats",
    allowedInAgent: true,
    requiresArgs: false,
  },
  {
    name: "ws",
    description: "slash.commands.ws.desc",
    usage: "/ws <工作区名称>",
    allowedInAgent: true,
    requiresArgs: true,
    argHint: "slash.commands.ws.argHint",
  },
  {
    name: "rename",
    description: "slash.commands.rename.desc",
    usage: "/rename <新标题>",
    allowedInAgent: true,
    requiresArgs: true,
    argHint: "slash.commands.rename.argHint",
  },
];

/**
 * 匹配斜杠命令
 * @param input 用户输入的完整文本（如 "/compact"、"/co"、"/ws my-project"）
 * @returns { exactMatch?: SlashCommand; fuzzyMatches: SlashCommand[] }
 *   - exactMatch: 精确匹配到的命令（输入恰好是 /command 形式，无额外参数）
 *   - fuzzyMatches: 模糊匹配的命令列表（命令名包含输入的子串）
 */
export function matchCommand(input: string): {
  exactMatch?: SlashCommand;
  fuzzyMatches: SlashCommand[];
} {
  // 输入必须以 / 开头，否则返回空模糊匹配
  if (!input.startsWith("/")) {
    return { fuzzyMatches: [] };
  }

  // 提取命令名部分（/ 后到第一个空格前的部分）
  const afterSlash = input.slice(1);
  const spaceIndex = afterSlash.indexOf(" ");
  const commandName = spaceIndex === -1 ? afterSlash : afterSlash.slice(0, spaceIndex);

  // 若无命令名（输入仅为 / 或 / 后紧跟空格），返回全部命令作为模糊匹配
  if (commandName === "") {
    return { fuzzyMatches: SLASH_COMMANDS };
  }

  // 精确匹配：输入恰好是 /command 形式（无额外参数）且命令名完全匹配
  const exactMatch = spaceIndex === -1
    ? SLASH_COMMANDS.find((cmd) => cmd.name === commandName)
    : undefined;

  // 模糊匹配：命令名包含输入的命令名子串（不区分大小写）
  const lowerInput = commandName.toLowerCase();
  const fuzzyMatches = SLASH_COMMANDS.filter((cmd) =>
    cmd.name.toLowerCase().includes(lowerInput)
  );

  return { exactMatch, fuzzyMatches };
}

/**
 * 解析斜杠命令的参数
 * @param input 用户输入的完整文本（如 "/ws my-project"）
 * @returns { command: string; args: string } | null
 *   - command: 命令名（不含 /）
 *   - args: 参数部分（命令名后的所有文本，已 trim）
 *   - 若输入不以 / 开头或格式不合法，返回 null
 */
export function parseCommandArgs(input: string): { command: string; args: string } | null {
  // 输入必须以 / 开头
  if (!input.startsWith("/")) {
    return null;
  }

  const afterSlash = input.slice(1);
  const spaceIndex = afterSlash.indexOf(" ");

  // 无空格：整体作为命令名，参数为空
  if (spaceIndex === -1) {
    const command = afterSlash;
    // 输入仅为 /，命令名为空，格式不合法
    if (command === "") {
      return null;
    }
    return { command, args: "" };
  }

  // 有空格：空格前为命令名，空格后为参数（保留中间空格，仅 trim 首尾）
  const command = afterSlash.slice(0, spaceIndex);
  const args = afterSlash.slice(spaceIndex + 1).trim();

  // 命令名为空，格式不合法
  if (command === "") {
    return null;
  }

  return { command, args };
}

/**
 * 按命令名获取命令定义
 * @param name 命令名（不含 / 前缀）
 * @returns 命令定义，未找到时返回 undefined
 */
export function getCommandByName(name: string): SlashCommand | undefined {
  return SLASH_COMMANDS.find((cmd) => cmd.name === name);
}
