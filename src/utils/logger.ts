type LogLevel = 'debug' | 'info' | 'warn' | 'error';

const LOG_LEVELS: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

const IS_DEV = import.meta.env.DEV;

const MIN_LEVEL: LogLevel = IS_DEV ? 'debug' : 'warn';

function getTimestamp(): string {
  const now = new Date();
  const h = String(now.getHours()).padStart(2, '0');
  const m = String(now.getMinutes()).padStart(2, '0');
  const s = String(now.getSeconds()).padStart(2, '0');
  const ms = String(now.getMilliseconds()).padStart(3, '0');
  return `${h}:${m}:${s}.${ms}`;
}

function shouldLog(level: LogLevel): boolean {
  return LOG_LEVELS[level] >= LOG_LEVELS[MIN_LEVEL];
}

class Logger {
  private module: string;

  constructor(module: string) {
    this.module = module;
  }

  debug(message: string, ...args: unknown[]): void {
    if (shouldLog('debug')) {
      console.debug(`[${getTimestamp()}] [${this.module}] [DEBUG] ${message}`, ...args);
    }
  }

  info(message: string, ...args: unknown[]): void {
    if (shouldLog('info')) {
      console.info(`[${getTimestamp()}] [${this.module}] [INFO] ${message}`, ...args);
    }
  }

  warn(message: string, ...args: unknown[]): void {
    if (shouldLog('warn')) {
      console.warn(`[${getTimestamp()}] [${this.module}] [WARN] ${message}`, ...args);
    }
  }

  error(message: string, ...args: unknown[]): void {
    if (shouldLog('error')) {
      console.error(`[${getTimestamp()}] [${this.module}] [ERROR] ${message}`, ...args);
    }
  }
}

/** 创建指定模块名的日志记录器 */
export function createLogger(module: string): Logger {
  return new Logger(module);
}
