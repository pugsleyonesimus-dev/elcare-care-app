const LEVELS: Record<string, number> = { debug: 10, info: 20, warn: 30, error: 40 };
const LOG_LEVEL = process.env.LOG_LEVEL || 'info';
const currentLevel = LEVELS[LOG_LEVEL] ?? 20;

function emit(level: string, numLevel: number, msg: string, fields?: Record<string, unknown>): void {
  if (numLevel < currentLevel) return;
  process.stdout.write(
    JSON.stringify({ level, time: Date.now(), msg, ...fields }) + '\n'
  );
}

export const logger = {
  debug: (msg: string, fields?: Record<string, unknown>) => emit('debug', 10, msg, fields),
  info:  (msg: string, fields?: Record<string, unknown>) => emit('info',  20, msg, fields),
  warn:  (msg: string, fields?: Record<string, unknown>) => emit('warn',  30, msg, fields),
  error: (msg: string, fields?: Record<string, unknown>) => emit('error', 40, msg, fields),
};
