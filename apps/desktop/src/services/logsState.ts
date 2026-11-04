import type { Tone } from "../components/tones";
import type { GuessedLevel, LogLine } from "../types/docker";

/** Line counts offered in the UI. The backend allows 1 to 1000. */
export const TAIL_OPTIONS = [100, 200, 500, 1000] as const;
export const DEFAULT_TAIL = 200;

export const LEVEL_TONE: Record<GuessedLevel, Tone> = {
  error: "bad",
  warn: "warn",
  info: "neutral",
  debug: "neutral",
};

/** Text colour for a line's message, by guessed level. */
export const LEVEL_TEXT: Record<GuessedLevel, string> = {
  error: "text-bad",
  warn: "text-warn",
  info: "text-fg",
  debug: "text-muted",
};

const ERROR = /\b(fatal|panic|critical|crit|emerg|error|err|exception|traceback)\b/i;
const WARN = /\b(warn|warning)\b/i;
const DEBUG = /\b(debug|trace|dbg)\b/i;
const INFO = /\b(info|notice)\b/i;

/**
 * Docker records no log level, so this is a guess from the text: it looks for the usual words
 * (including `level=error` and `"level":"warn"` styles). It will sometimes be wrong, for example
 * on "no error found", so the UI says it is a guess.
 */
export function guessLevel(message: string): GuessedLevel | null {
  if (ERROR.test(message)) return "error";
  if (WARN.test(message)) return "warn";
  if (DEBUG.test(message)) return "debug";
  if (INFO.test(message)) return "info";
  return null;
}

export type LevelFilter = "all" | GuessedLevel;

/** Keeps lines that contain `query` (plain text, any case) and match the level. */
export function filterLogLines(lines: LogLine[], query: string, level: LevelFilter): LogLine[] {
  const q = query.trim().toLowerCase();
  if (q === "" && level === "all") return lines;
  return lines.filter(
    (l) =>
      (level === "all" || guessLevel(l.message) === level) &&
      (q === "" || l.message.toLowerCase().includes(q)),
  );
}

export interface LevelCounts {
  error: number;
  warn: number;
}

export function countLevels(lines: LogLine[]): LevelCounts {
  let error = 0;
  let warn = 0;
  for (const l of lines) {
    const level = guessLevel(l.message);
    if (level === "error") error++;
    else if (level === "warn") warn++;
  }
  return { error, warn };
}

const pad = (n: number, width = 2) => String(n).padStart(width, "0");

/** "14:03:07.512" in local time, or "" when the line has no usable timestamp. */
export function formatLogTime(time: string | undefined): string {
  if (time === undefined) return "";
  const ms = Date.parse(time);
  if (Number.isNaN(ms)) return "";
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${pad(d.getMilliseconds(), 3)}`;
}
