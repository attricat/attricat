/**
 * A tolerant YAML outline for editor features. It never throws: incomplete
 * input, which is the normal state while typing, yields a best-effort outline
 * of keys and values with their source ranges, plus the cursor's context.
 * Exact parsing and syntax errors come from `parseStrictYaml`.
 */
import {
  isMap,
  isScalar,
  isSeq,
  parseDocument,
  type Node,
  type Pair,
} from 'yaml';

export type YamlPath = (string | number)[];
export type TextRange = { start: number; end: number };
type YamlScalar = string | number | boolean;

export type YamlEntry = {
  path: YamlPath;
  /** The key, or the element itself for sequence items. */
  range: TextRange;
  value?: YamlScalar;
  /** The scalar value as written, including any quotes. */
  valueRange?: TextRange;
};

/** A mapping or sequence and where its content sits in the text. */
export type YamlCollection = {
  /** Whether it is written in flow style, `[…]` or `{…}`. */
  flow: boolean;
  kind: 'map' | 'seq';
  path: YamlPath;
  range: TextRange;
};

export type YamlCursor =
  | {
      kind: 'key';
      /** Inside a flow mapping, where keys are separated by commas. */
      inline: boolean;
      /** Right after a `- `, so the key starts a new sequence item. */
      item: boolean;
      /** The mapping that receives the key. */
      mapping: YamlPath;
      range: TextRange;
    }
  | {
      /** Inside a flow collection, where commas and brackets need quotes. */
      flow: boolean;
      kind: 'value';
      path: YamlPath;
      quoted: boolean;
      range: TextRange;
    };

export type YamlOutline = {
  collections: YamlCollection[];
  cursor?: YamlCursor;
  entries: YamlEntry[];
  /** Direct child keys already present in a mapping. */
  keysIn: (mapping: YamlPath) => Set<string>;
  /** The entry at exactly this path. */
  entryAt: (path: YamlPath) => YamlEntry | undefined;
  /** Source range for a path, falling back to its nearest ancestor. */
  rangeOf: (path: YamlPath) => TextRange | undefined;
  valueAt: (path: YamlPath) => YamlScalar | undefined;
};

const pathKey = (path: YamlPath) => JSON.stringify(path);

const toRange = (range: [number, number, number] | null | undefined) =>
  range ? { end: range[1], start: range[0] } : undefined;

const scalarValue = (node: unknown): YamlScalar | undefined => {
  if (!isScalar(node)) return undefined;
  const { value } = node;
  return typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
    ? value
    : undefined;
};

const keyName = (pair: Pair) => {
  const key = isScalar(pair.key) ? pair.key.value : pair.key;
  return key === null || key === undefined ? undefined : String(key);
};

/** Walks the document's nodes, recording each key, item, and collection. */
const collect = (
  text: string,
  node: unknown,
  path: YamlPath,
  entries: YamlEntry[],
  collections: YamlCollection[],
) => {
  if (isMap(node)) {
    const range = toRange(node.range);
    if (range)
      collections.push({ flow: Boolean(node.flow), kind: 'map', path, range });
    for (const pair of node.items) {
      const name = keyName(pair);
      const keyRange = isScalar(pair.key) ? toRange(pair.key.range) : undefined;
      if (name === undefined || !keyRange) continue;
      const childPath = [...path, name];
      const value = scalarValue(pair.value);
      const valueRange =
        value === undefined
          ? undefined
          : toRange((pair.value as Node | null)?.range);
      entries.push({ path: childPath, range: keyRange, value, valueRange });
      collect(text, pair.value, childPath, entries, collections);
    }
  } else if (isSeq(node)) {
    const range = toRange(node.range);
    if (range)
      collections.push({ flow: Boolean(node.flow), kind: 'seq', path, range });
    node.items.forEach((item, index) => {
      const childPath = [...path, index];
      const itemRange = toRange((item as Node | null)?.range);
      if (itemRange) {
        const value = scalarValue(item);
        // A block item is located by its first line, as a key is by its name.
        const firstLineEnd = text.indexOf('\n', itemRange.start);
        entries.push({
          path: childPath,
          range:
            value === undefined && firstLineEnd >= 0
              ? { ...itemRange, end: Math.min(itemRange.end, firstLineEnd) }
              : itemRange,
          value,
          valueRange: value === undefined ? undefined : itemRange,
        });
      }
      collect(text, item, childPath, entries, collections);
    });
  }
};

type Line = { start: number; end: number; text: string };

const lineAt = (text: string, offset: number): Line => {
  const start = text.lastIndexOf('\n', offset - 1) + 1;
  const newline = text.indexOf('\n', offset);
  const end = newline < 0 ? text.length : newline;
  return { end, start, text: text.slice(start, end) };
};

const previousLine = (text: string, line: Line): Line | undefined =>
  line.start === 0 ? undefined : lineAt(text, line.start - 1);

/** Indentation, `- ` item markers, and the column where content starts. */
const lineShape = (text: string) => {
  const indent = text.length - text.trimStart().length;
  let content = indent;
  const dashes: number[] = [];
  while (
    text[content] === '-' &&
    (content + 1 === text.length || text[content + 1] === ' ')
  ) {
    dashes.push(content);
    content++;
    while (text[content] === ' ') content++;
  }
  return { content, dashes, indent };
};

const isBlank = (text: string) => {
  const trimmed = text.trim();
  return trimmed === '' || trimmed.startsWith('#');
};

/** The end of the word at `offset`, for replacing what is already typed. */
const wordEnd = (text: string, offset: number, pattern: RegExp) => {
  let end = offset;
  while (end < text.length && pattern.test(text[end]!)) end++;
  return end;
};

const keyCharacter = /[A-Za-z0-9_.:-]/;
const plainCharacter = /[^\s,\]}#]/;

/** Splits `key: value` text at its mapping colon, honoring quotes. */
const colonIndex = (text: string) => {
  let quote: string | undefined;
  for (let index = 0; index < text.length; index++) {
    const character = text[index]!;
    if (quote) {
      if (character === quote) quote = undefined;
      continue;
    }
    if (character === '"' || character === "'") quote = character;
    else if (character === '#' && (index === 0 || text[index - 1] === ' '))
      return -1;
    else if (
      character === ':' &&
      (index + 1 === text.length || text[index + 1] === ' ')
    )
      return index;
    else if (character === '[' || character === '{') return -1;
  }
  return -1;
};

const unquote = (key: string) => {
  const trimmed = key.trim();
  return /^(["']).*\1$/.test(trimmed) ? trimmed.slice(1, -1) : trimmed;
};

class CursorFinder {
  constructor(
    private readonly text: string,
    private readonly offset: number,
    private readonly entries: YamlEntry[],
  ) {}

  /**
   * The entry whose key or item starts at this offset. A mapping item starts
   * where its first key does; the key is preferred.
   */
  private entryStartingAt(offset: number) {
    const starting = this.entries.filter(({ range }) => range.start === offset);
    return (
      starting.find(({ path }) => typeof path.at(-1) === 'string') ??
      starting[0]
    );
  }

  /**
   * The mapping that a key written at `column` on `line` belongs to: the
   * mapping of a key above it in the same column, or the `key:` or `- `
   * item that opens a block above it at a lower indentation.
   */
  private mappingAt(line: Line, column: number): YamlPath | undefined {
    for (
      let above = previousLine(this.text, line);
      above;
      above = previousLine(this.text, above)
    ) {
      if (isBlank(above.text)) continue;
      const sibling = this.entryStartingAt(above.start + column);
      if (sibling && typeof sibling.path.at(-1) === 'string')
        return sibling.path.slice(0, -1);
      const shape = lineShape(above.text);
      if (shape.content >= column) continue;
      const opener = this.entryStartingAt(above.start + shape.content);
      // `key:` with nothing after it, or an empty `- ` item, opens the block.
      return opener && opener.value === undefined ? opener.path : undefined;
    }
    return column === 0 ? [] : undefined;
  }

  /** The path of a new `- ` item whose dash is at column `dash`. */
  private itemAt(line: Line, dash: number): YamlPath | undefined {
    for (
      let above = previousLine(this.text, line);
      above;
      above = previousLine(this.text, above)
    ) {
      if (isBlank(above.text)) continue;
      const shape = lineShape(above.text);
      const dashIndex = shape.dashes.indexOf(dash);
      if (dashIndex >= 0) {
        // The previous item, a scalar or a mapping whose first key follows
        // the dash.
        const itemStart =
          above.start + (shape.dashes[dashIndex + 1] ?? shape.content);
        const previous = this.entries.find(
          ({ path, range }) =>
            range.start === itemStart && typeof path.at(-1) === 'number',
        );
        const firstKey = this.entryStartingAt(itemStart);
        const itemPath =
          previous?.path ??
          (firstKey && typeof firstKey.path.at(-2) === 'number'
            ? firstKey.path.slice(0, -1)
            : undefined);
        if (!itemPath) return undefined;
        return [...itemPath.slice(0, -1), (itemPath.at(-1) as number) + 1];
      }
      if (shape.content > dash) continue;
      // `key:` opening the sequence, above or level with its dashes.
      const opener = this.entryStartingAt(above.start + shape.content);
      return opener && opener.value === undefined
        ? [...opener.path, 0]
        : undefined;
    }
    return undefined;
  }

  find(): YamlCursor | undefined {
    const line = lineAt(this.text, this.offset);
    const column = this.offset - line.start;
    const shape = lineShape(line.text);
    if (column < shape.indent && line.text.trim()) return undefined;
    const before = line.text.slice(
      shape.content,
      Math.max(column, shape.content),
    );
    if (/(^|\s)#/.test(before.replace(/(["']).*?\1/g, ''))) return undefined;

    // A `-` needs its space before an item's content can follow it.
    if (shape.dashes.length && line.text[shape.content - 1] === '-')
      return undefined;
    const colon = colonIndex(line.text.slice(shape.content));
    const contentStart = line.start + shape.content;
    // The cursor is on the key, or where a new key goes.
    if (colon < 0 || column <= shape.content + colon) {
      const typed = line.text.slice(shape.content, column);
      if (colon < 0 && !/^[A-Za-z0-9_"'.:-]*$/.test(typed)) return undefined;
      const item = shape.dashes.length > 0;
      const mapping = this.keyMapping(line, shape, item);
      if (!mapping) return undefined;
      const quoted = /^["']/.test(typed);
      const start = contentStart + (quoted ? 1 : 0);
      return {
        inline: false,
        item: item && column >= shape.content,
        kind: 'key',
        mapping,
        range: {
          end: Math.max(
            wordEnd(this.text, this.offset, keyCharacter),
            this.offset,
          ),
          start: Math.min(start, this.offset),
        },
      };
    }

    const keyText = line.text.slice(shape.content, shape.content + colon);
    const keyEntry = this.entryStartingAt(contentStart);
    const keyPath =
      keyEntry?.path ??
      (() => {
        const mapping = this.keyMapping(line, shape, shape.dashes.length > 0);
        return mapping ? [...mapping, unquote(keyText)] : undefined;
      })();
    if (!keyPath) return undefined;
    const valueStart = line.start + shape.content + colon + 1;
    return this.valueCursor(keyPath, valueStart, line.end);
  }

  private keyMapping(
    line: Line,
    shape: ReturnType<typeof lineShape>,
    item: boolean,
  ) {
    const existing = this.entryStartingAt(line.start + shape.content);
    if (existing && typeof existing.path.at(-1) === 'string')
      return existing.path.slice(0, -1);
    if (!item) return this.mappingAt(line, shape.content);
    // `- ` alone, or a new item key: the item is a mapping of its sequence.
    const dash = shape.dashes.at(-1)!;
    if (existing && typeof existing.path.at(-1) === 'number')
      return existing.path;
    const nullItem = this.entries.find(
      ({ path, range }) =>
        typeof path.at(-1) === 'number' &&
        range.start >= line.start + dash &&
        range.start <= line.end,
    );
    return nullItem?.path ?? this.itemAt(line, dash);
  }

  /** The value context after `key:`, block or flow. */
  private valueCursor(
    keyPath: YamlPath,
    valueStart: number,
    lineEnd: number,
  ): YamlCursor | undefined {
    // A value needs the space after its key's colon.
    if (this.text[valueStart] !== ' ') return undefined;
    let position = valueStart;
    while (this.text[position] === ' ') position++;
    if (this.offset < position)
      return {
        flow: false,
        kind: 'value',
        path: keyPath,
        quoted: false,
        range: { end: this.offset, start: this.offset },
      };
    const opener = this.text[position];
    if (opener === '[' || opener === '{')
      return this.flowCursor(keyPath, position, lineEnd);
    return this.scalarCursor(keyPath, position, lineEnd);
  }

  private scalarCursor(
    path: YamlPath,
    start: number,
    lineEnd: number,
  ): YamlCursor | undefined {
    const quote = this.text[start];
    if (quote === '"' || quote === "'") {
      const close = this.text.indexOf(quote, start + 1);
      const end = close < 0 || close > lineEnd ? lineEnd : close;
      if (this.offset > end && close >= 0) return undefined;
      return {
        flow: false,
        kind: 'value',
        path,
        quoted: true,
        range: { end, start: start + 1 },
      };
    }
    if (quote === '|' || quote === '>') return undefined;
    const end = wordEnd(this.text, start, /[^\n#]/);
    const valueEnd = this.text.slice(start, end).trimEnd().length + start;
    if (this.offset > valueEnd && this.offset !== start) return undefined;
    return {
      flow: false,
      kind: 'value',
      path,
      quoted: false,
      range: { end: Math.max(valueEnd, this.offset), start },
    };
  }

  /** Walks a flow collection up to the cursor, tracking nesting. */
  private flowCursor(
    path: YamlPath,
    start: number,
    lineEnd: number,
  ): YamlCursor | undefined {
    type Frame = {
      kind: 'map' | 'seq';
      path: YamlPath;
      index: number;
      key?: string;
      /** Where the current element or key started. */
      elementStart: number;
      afterColon: boolean;
    };
    const stack: Frame[] = [];
    let position = start;
    const open = (kind: 'map' | 'seq', framePath: YamlPath) =>
      stack.push({
        afterColon: false,
        elementStart: position + 1,
        index: 0,
        kind,
        path: framePath,
      });
    const elementPath = (frame: Frame) =>
      frame.kind === 'seq'
        ? [...frame.path, frame.index]
        : [...frame.path, frame.key ?? ''];
    while (position < this.offset && position < lineEnd) {
      const character = this.text[position]!;
      const frame = stack.at(-1);
      if (character === '"' || character === "'") {
        const close = this.text.indexOf(character, position + 1);
        if (close < 0 || close >= this.offset) {
          if (!frame) return undefined;
          const end = close < 0 || close > lineEnd ? lineEnd : close;
          if (frame.kind === 'map' && !frame.afterColon)
            return {
              inline: true,
              item: false,
              kind: 'key',
              mapping: frame.path,
              range: { end, start: position + 1 },
            };
          return {
            flow: true,
            kind: 'value',
            path: elementPath(frame),
            quoted: true,
            range: { end, start: position + 1 },
          };
        }
        position = close + 1;
        continue;
      }
      if (character === '[' || character === '{') {
        const childPath = frame ? elementPath(frame) : path;
        open(character === '[' ? 'seq' : 'map', childPath);
      } else if (character === ']' || character === '}') {
        stack.pop();
        if (!stack.length) return undefined;
      } else if (frame && character === ',') {
        frame.index++;
        frame.key = undefined;
        frame.afterColon = false;
        frame.elementStart = position + 1;
      } else if (
        frame?.kind === 'map' &&
        !frame.afterColon &&
        character === ':'
      ) {
        frame.key = unquote(this.text.slice(frame.elementStart, position));
        frame.afterColon = true;
        frame.elementStart = position + 1;
      }
      position++;
    }
    const frame = stack.at(-1);
    if (!frame) return undefined;
    let elementStart = frame.elementStart;
    while (this.text[elementStart] === ' ') elementStart++;
    const end = Math.max(
      wordEnd(this.text, this.offset, plainCharacter),
      this.offset,
    );
    const range = { end, start: Math.min(elementStart, this.offset) };
    if (frame.kind === 'map' && !frame.afterColon)
      return {
        inline: true,
        item: false,
        kind: 'key',
        mapping: frame.path,
        range,
      };
    return {
      flow: true,
      kind: 'value',
      path: elementPath(frame),
      quoted: false,
      range,
    };
  }
}

/**
 * The text with the cursor line's value blanked, so that an unclosed quote or
 * bracket being typed there does not swallow the lines after it. Offsets are
 * unchanged.
 */
const withoutCursorValue = (text: string, cursor: number | undefined) => {
  if (cursor === undefined) return text;
  const line = lineAt(text, cursor);
  const shape = lineShape(line.text);
  const colon = colonIndex(line.text.slice(shape.content));
  if (colon < 0) return text;
  const valueStart = line.start + shape.content + colon + 1;
  return (
    text.slice(0, valueStart) +
    ' '.repeat(line.end - valueStart) +
    text.slice(line.end)
  );
};

/** The document, reparsed without the cursor's value when that helps. */
const parseOutline = (text: string, cursor: number | undefined) => {
  const document = parseDocument(text, { uniqueKeys: false });
  if (!document.errors.length || cursor === undefined) return document;
  const recovered = parseDocument(withoutCursorValue(text, cursor), {
    uniqueKeys: false,
  });
  return recovered.errors.length < document.errors.length
    ? recovered
    : document;
};

export const scanYaml = (text: string, cursor?: number): YamlOutline => {
  const entries: YamlEntry[] = [];
  const collections: YamlCollection[] = [];
  try {
    collect(
      text,
      parseOutline(text, cursor).contents,
      [],
      entries,
      collections,
    );
  } catch {
    // The outline stays empty for input the parser cannot represent.
    entries.length = collections.length = 0;
  }
  const byPath = new Map(entries.map((entry) => [pathKey(entry.path), entry]));
  const keys = new Map<string, Set<string>>();
  for (const { path } of entries) {
    const last = path.at(-1);
    if (typeof last !== 'string') continue;
    const parent = pathKey(path.slice(0, -1));
    if (!keys.has(parent)) keys.set(parent, new Set());
    keys.get(parent)!.add(last);
  }
  return {
    collections,
    cursor:
      cursor === undefined
        ? undefined
        : new CursorFinder(text, cursor, entries).find(),
    entries,
    entryAt: (path) => byPath.get(pathKey(path)),
    keysIn: (mapping) => keys.get(pathKey(mapping)) ?? new Set(),
    rangeOf: (path) => {
      for (let length = path.length; length > 0; length--) {
        const entry = byPath.get(pathKey(path.slice(0, length)));
        if (entry) return entry.range;
      }
      return undefined;
    },
    valueAt: (path) => byPath.get(pathKey(path))?.value,
  };
};
