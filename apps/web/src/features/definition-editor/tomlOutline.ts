/**
 * A tolerant TOML scanner for editor features. It never throws: incomplete
 * input, which is the normal state while typing, yields a best-effort outline
 * of keys and values with their source ranges, plus the cursor's context.
 * Exact parsing and syntax errors come from `smol-toml`.
 */

export type TomlPath = (string | number)[];
export type TextRange = { start: number; end: number };
export type TomlScalar = string | number | boolean;

export type TomlEntry = {
  path: TomlPath;
  /** The key, or the element itself for array items. */
  range: TextRange;
  value?: TomlScalar;
};

export type TomlHeader = {
  array: boolean;
  path: TomlPath;
  range: TextRange;
};

export type TomlCursor =
  | {
      kind: 'key';
      inline: boolean;
      range: TextRange;
      /** The table that receives the key, including dotted-key prefixes. */
      table: TomlPath;
    }
  | { kind: 'value'; path: TomlPath; quoted: boolean; range: TextRange }
  | { kind: 'header'; array: boolean; range: TextRange; table: TomlPath };

export type TomlOutline = {
  cursor?: TomlCursor;
  entries: TomlEntry[];
  headers: TomlHeader[];
  /** Direct child keys already present in a table. */
  keysIn: (table: TomlPath) => Set<string>;
  /** Source range for a path, falling back to its nearest ancestor. */
  rangeOf: (path: TomlPath) => TextRange | undefined;
  valueAt: (path: TomlPath) => TomlScalar | undefined;
};

const pathKey = (path: TomlPath) => JSON.stringify(path);

const isBareKeyCharacter = (character: string | undefined) =>
  character !== undefined && /[A-Za-z0-9_-]/.test(character);

const isInlineSpace = (character: string | undefined) =>
  character === ' ' || character === '\t';

const isLineEnd = (character: string | undefined) =>
  character === undefined || character === '\n' || character === '\r';

const headerLine = /^[ \t]*\[\[?[ \t]*[A-Za-z0-9_"'.\- \t]+\]\]?[ \t]*(#.*)?$/;
const keyValueLine = /^[ \t]*[A-Za-z0-9_"'.-]+[ \t]*=/;
const numberLiteral = /^[+-]?(\d[\d_]*)(\.\d[\d_]*)?([eE][+-]?\d+)?$/;

type KeySegment = { name: string; range: TextRange };

class Scanner {
  cursorContext?: TomlCursor;
  readonly entries: TomlEntry[] = [];
  readonly headers: TomlHeader[] = [];
  private readonly arrayCounts = new Map<string, number>();
  private readonly arrayTables = new Set<string>();
  private position = 0;
  /** Set when an unterminated collection stops at the start of a new line. */
  private resumeAtLineStart = false;
  private table: TomlPath = [];
  private readonly text: string;
  private readonly cursor: number | undefined;

  constructor(text: string, cursor: number | undefined) {
    this.text = text;
    this.cursor = cursor;
  }

  scan() {
    while (this.position <= this.text.length) {
      const lineBegin = this.position;
      this.skipInlineSpace();
      const character = this.peek();
      if (isLineEnd(character) || character === '#') {
        this.captureBlankLine(lineBegin);
        this.skipLine();
        if (!this.skipNewline()) break;
        continue;
      }
      if (character === '[') this.header();
      else this.keyValue(this.table, false);
      if (this.resumeAtLineStart) {
        this.resumeAtLineStart = false;
        continue;
      }
      this.skipLine();
      if (!this.skipNewline()) break;
    }
  }

  private captureBlankLine(lineBegin: number) {
    if (this.cursor === undefined || this.cursorContext) return;
    if (this.cursor < lineBegin || this.cursor > this.position) return;
    this.cursorContext = {
      inline: false,
      kind: 'key',
      range: { end: this.cursor, start: this.cursor },
      table: this.table,
    };
  }

  private header() {
    const start = this.position;
    const array = this.text.startsWith('[[', start);
    this.position += array ? 2 : 1;
    const contentStart = this.position;
    this.skipInlineSpace();
    const segments = this.key();
    this.skipInlineSpace();
    while (!isLineEnd(this.peek()) && this.peek() !== ']') this.position++;
    const contentEnd = this.position;
    const closing = array ? ']]' : ']';
    if (this.text.startsWith(closing, this.position))
      this.position += closing.length;
    if (
      this.cursor !== undefined &&
      !this.cursorContext &&
      this.cursor >= contentStart &&
      this.cursor <= contentEnd
    ) {
      this.cursorContext = {
        array,
        kind: 'header',
        range: { end: contentEnd, start: contentStart },
        table: this.table,
      };
    }
    if (!segments.length) return;
    const path = this.resolveHeader(
      segments.map((segment) => segment.name),
      array,
    );
    this.headers.push({
      array,
      path,
      range: { end: this.position, start },
    });
    this.table = path;
  }

  private resolveHeader(names: string[], array: boolean) {
    const path: TomlPath = [];
    names.forEach((name, index) => {
      path.push(name);
      const key = pathKey(path);
      if (index === names.length - 1) {
        if (!array) return;
        const next = (this.arrayCounts.get(key) ?? -1) + 1;
        this.arrayCounts.set(key, next);
        this.arrayTables.add(key);
        path.push(next);
      } else if (this.arrayTables.has(key)) {
        path.push(this.arrayCounts.get(key) ?? 0);
      }
    });
    return path;
  }

  /** Parses a possibly dotted key; a trailing dot yields an empty segment. */
  private key(): KeySegment[] {
    const segments: KeySegment[] = [];
    for (;;) {
      const start = this.position;
      const character = this.peek();
      if (character === '"' || character === "'") {
        const { content, contentRange } = this.string();
        segments.push({ name: content, range: contentRange });
      } else if (isBareKeyCharacter(character)) {
        while (isBareKeyCharacter(this.peek())) this.position++;
        segments.push({
          name: this.text.slice(start, this.position),
          range: { end: this.position, start },
        });
      } else {
        if (segments.length)
          segments.push({ name: '', range: { end: start, start } });
        return segments;
      }
      const afterSegment = this.position;
      this.skipInlineSpace();
      if (this.peek() !== '.') {
        this.position = afterSegment;
        return segments;
      }
      this.position++;
      this.skipInlineSpace();
    }
  }

  private keyValue(base: TomlPath, inline: boolean) {
    const segments = this.key();
    if (!segments.length) return false;
    const names = segments.map((segment) => segment.name);
    if (this.cursor !== undefined && !this.cursorContext) {
      const index = segments.findIndex(
        (segment) =>
          this.cursor! >= segment.range.start &&
          this.cursor! <= segment.range.end,
      );
      if (index >= 0) {
        this.cursorContext = {
          inline,
          kind: 'key',
          range: segments[index].range,
          table: [...base, ...names.slice(0, index)],
        };
      }
    }
    this.skipInlineSpace();
    if (this.peek() !== '=') return true;
    this.position++;
    const path = [...base, ...names];
    const keyRange = segments[segments.length - 1].range;
    this.value(path, keyRange, inline);
    return true;
  }

  private value(
    path: TomlPath,
    keyRange: TextRange | undefined,
    inline: boolean,
  ) {
    const slotStart = this.position;
    this.skipInlineSpace();
    const start = this.position;
    const character = this.peek();
    const cursorInSlot =
      this.cursor !== undefined &&
      !this.cursorContext &&
      this.cursor >= slotStart &&
      this.cursor <= start;
    if (
      cursorInSlot &&
      (isLineEnd(character) ||
        character === '#' ||
        character === ',' ||
        character === ']' ||
        character === '}')
    ) {
      this.cursorContext = {
        kind: 'value',
        path,
        quoted: false,
        range: { end: this.cursor!, start: this.cursor! },
      };
    }
    if (character === '"' || character === "'") {
      const { content, contentRange } = this.string();
      this.entries.push({
        path,
        range: keyRange ?? { end: this.position, start },
        value: content,
      });
      if (
        this.cursor !== undefined &&
        !this.cursorContext &&
        this.cursor >= contentRange.start &&
        this.cursor <= contentRange.end
      ) {
        this.cursorContext = {
          kind: 'value',
          path,
          quoted: true,
          range: contentRange,
        };
      }
      return;
    }
    if (character === '[') {
      this.entries.push({ path, range: keyRange ?? { end: start + 1, start } });
      this.array(path);
      return;
    }
    if (character === '{') {
      this.entries.push({ path, range: keyRange ?? { end: start + 1, start } });
      this.inlineTable(path);
      return;
    }
    while (
      !isLineEnd(this.peek()) &&
      !isInlineSpace(this.peek()) &&
      !(inline && this.peek() === '}') &&
      !['#', ',', ']'].includes(this.peek() ?? '')
    )
      this.position++;
    const raw = this.text.slice(start, this.position);
    if (!raw) return;
    this.entries.push({
      path,
      range: keyRange ?? { end: this.position, start },
      value: scalar(raw),
    });
    if (
      this.cursor !== undefined &&
      !this.cursorContext &&
      this.cursor >= start &&
      this.cursor <= this.position
    ) {
      this.cursorContext = {
        kind: 'value',
        path,
        quoted: false,
        range: { end: this.position, start },
      };
    }
  }

  private array(path: TomlPath) {
    this.position++;
    let index = 0;
    let slotStart = this.position;
    for (;;) {
      const open = this.skipSpaceInCollection(true);
      const character = open ? this.peek() : undefined;
      if (
        this.cursor !== undefined &&
        !this.cursorContext &&
        this.cursor >= slotStart &&
        this.cursor <= this.position &&
        (character === ']' || character === ',' || character === undefined)
      ) {
        this.cursorContext = {
          kind: 'value',
          path: [...path, index],
          quoted: false,
          range: { end: this.cursor, start: this.cursor },
        };
      }
      if (character === undefined) return;
      if (character === ']') {
        this.position++;
        return;
      }
      if (character !== ',') {
        const before = this.position;
        this.value([...path, index], undefined, true);
        if (this.resumeAtLineStart) return;
        if (this.position === before) this.position++;
        if (!this.skipSpaceInCollection(true)) return;
      }
      if (this.peek() === ',') {
        this.position++;
        index++;
        slotStart = this.position;
      } else if (this.peek() === ']') {
        this.position++;
        return;
      } else if (this.peek() === undefined) {
        return;
      }
    }
  }

  private inlineTable(path: TomlPath) {
    this.position++;
    let slotStart = this.position;
    for (;;) {
      const open = this.skipSpaceInCollection(false);
      const character = open ? this.peek() : undefined;
      if (
        this.cursor !== undefined &&
        !this.cursorContext &&
        this.cursor >= slotStart &&
        this.cursor <= this.position &&
        (character === '}' || character === ',' || character === undefined)
      ) {
        this.cursorContext = {
          inline: true,
          kind: 'key',
          range: { end: this.cursor, start: this.cursor },
          table: path,
        };
      }
      if (character === undefined) return;
      if (character === '}') {
        this.position++;
        return;
      }
      if (character !== ',') {
        const before = this.position;
        this.keyValue(path, true);
        if (this.resumeAtLineStart) return;
        if (this.position === before) this.position++;
        if (!this.skipSpaceInCollection(false)) return;
      }
      if (this.peek() === ',') {
        this.position++;
        slotStart = this.position;
      } else if (this.peek() === '}') {
        this.position++;
        return;
      } else if (this.peek() === undefined) {
        return;
      }
    }
  }

  /**
   * Skips whitespace, newlines, and comments inside `[...]` or `{...}`.
   * Returns false when the next line starts a new header or key, which means
   * the collection was left unterminated while typing.
   */
  private skipSpaceInCollection(array: boolean) {
    for (;;) {
      const character = this.peek();
      if (isInlineSpace(character)) {
        this.position++;
      } else if (character === '#') {
        this.skipLine();
      } else if (character === '\n' || character === '\r') {
        this.skipNewline();
        const lineEnd = this.text.indexOf('\n', this.position);
        const line = this.text.slice(
          this.position,
          lineEnd < 0 ? undefined : lineEnd,
        );
        if (headerLine.test(line) || (array && keyValueLine.test(line))) {
          this.resumeAtLineStart = true;
          return false;
        }
      } else {
        return true;
      }
    }
  }

  private string() {
    const quote = this.peek()!;
    const multiline = this.text.startsWith(quote.repeat(3), this.position);
    const delimiter = multiline ? quote.repeat(3) : quote;
    this.position += delimiter.length;
    const start = this.position;
    let content = '';
    for (;;) {
      const character = this.peek();
      if (character === undefined || (!multiline && isLineEnd(character))) {
        return { content, contentRange: { end: this.position, start } };
      }
      if (this.text.startsWith(delimiter, this.position)) {
        const end = this.position;
        this.position += delimiter.length;
        return { content, contentRange: { end, start } };
      }
      if (character === '\\' && quote === '"') {
        content += this.text[this.position + 1] ?? '';
        this.position += 2;
        continue;
      }
      content += character;
      this.position++;
    }
  }

  private peek(): string | undefined {
    return this.text[this.position];
  }

  private skipInlineSpace() {
    while (isInlineSpace(this.peek())) this.position++;
  }

  private skipLine() {
    while (!isLineEnd(this.peek())) this.position++;
  }

  private skipNewline() {
    if (this.peek() === '\r') this.position++;
    if (this.peek() === '\n') {
      this.position++;
      return true;
    }
    return this.position < this.text.length;
  }
}

const scalar = (raw: string): TomlScalar => {
  if (raw === 'true') return true;
  if (raw === 'false') return false;
  if (numberLiteral.test(raw)) return Number(raw.replaceAll('_', ''));
  return raw;
};

const startsWith = (path: TomlPath, prefix: TomlPath) =>
  prefix.every((segment, index) => path[index] === segment);

export const scanToml = (text: string, cursor?: number): TomlOutline => {
  const scanner = new Scanner(text, cursor);
  scanner.scan();
  const { entries, headers } = scanner;
  const values = new Map<string, TomlScalar>();
  const ranges = new Map<string, TextRange>();
  for (const header of headers) ranges.set(pathKey(header.path), header.range);
  for (const entry of entries) {
    if (entry.value !== undefined) values.set(pathKey(entry.path), entry.value);
    ranges.set(pathKey(entry.path), entry.range);
  }
  return {
    cursor: scanner.cursorContext,
    entries,
    headers,
    keysIn: (table) => {
      const keys = new Set<string>();
      for (const { path } of [...entries, ...headers]) {
        const key = path[table.length];
        if (
          path.length > table.length &&
          typeof key === 'string' &&
          startsWith(path, table)
        )
          keys.add(key);
      }
      return keys;
    },
    rangeOf: (path) => {
      for (let length = path.length; length > 0; length--) {
        const range = ranges.get(pathKey(path.slice(0, length)));
        if (range) return range;
      }
      return undefined;
    },
    valueAt: (path) => values.get(pathKey(path)),
  };
};
