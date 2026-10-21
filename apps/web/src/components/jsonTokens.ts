export type JsonTokenKind = 'key' | 'punctuation' | 'string' | 'literal';

export type JsonToken = { kind: JsonTokenKind; text: string };

export type JsonContainer = unknown[] | Record<string, unknown>;

export const isJsonContainer = (value: unknown): value is JsonContainer =>
  typeof value === 'object' && value !== null;

/** Child entries of an object or array, keyed by property name or index. */
export const jsonEntries = (value: JsonContainer): [string, unknown][] =>
  Array.isArray(value)
    ? value.map((item, index) => [String(index), item])
    : Object.entries(value);

export const jsonBrackets = (value: JsonContainer) =>
  Array.isArray(value) ? (['[', ']'] as const) : (['{', '}'] as const);

/** JSON text of a scalar; values JSON cannot hold fall back to `String`. */
export const jsonScalarText = (value: unknown) =>
  JSON.stringify(value) ?? String(value);

/** Ends a truncated token list. */
export const jsonEllipsis: JsonToken = { kind: 'punctuation', text: '…' };

/**
 * One-line JSON split into tokens so keys and punctuation can be set apart
 * from values. Stops after about `maxLength` characters, ending in an ellipsis,
 * so a large value in a table cell does not render in full.
 */
export const jsonInlineTokens = (
  value: unknown,
  maxLength = Number.POSITIVE_INFINITY,
): JsonToken[] => {
  const tokens: JsonToken[] = [];
  let length = 0;
  let truncated = false;
  const push = (kind: JsonTokenKind, text: string) => {
    tokens.push({ kind, text });
    length += text.length;
  };
  const full = () => {
    if (length < maxLength) return false;
    truncated = true;
    return true;
  };
  const visit = (item: unknown) => {
    if (!isJsonContainer(item)) {
      const kind = typeof item === 'string' ? 'string' : 'literal';
      const text = jsonScalarText(item);
      const room = Math.max(maxLength - length, 1);
      if (text.length <= room) {
        push(kind, text);
        return;
      }
      push(kind, text.slice(0, room));
      tokens.push(jsonEllipsis);
      truncated = true;
      return;
    }
    const [open, close] = jsonBrackets(item);
    const entries = jsonEntries(item);
    if (!entries.length) {
      push('punctuation', `${open}${close}`);
      return;
    }
    push('punctuation', Array.isArray(item) ? open : `${open} `);
    for (const [index, [key, child]] of entries.entries()) {
      if (index) push('punctuation', ', ');
      if (full()) break;
      if (!Array.isArray(item)) {
        push('key', JSON.stringify(key));
        push('punctuation', ': ');
      }
      visit(child);
      if (truncated) return;
    }
    if (truncated) {
      tokens.push(jsonEllipsis);
      return;
    }
    push('punctuation', Array.isArray(item) ? close : ` ${close}`);
  };
  visit(value);
  return tokens;
};

/** Pretty-printed JSON, as copied to the clipboard. */
export const formatJson = (value: unknown) =>
  JSON.stringify(value, null, 2) ?? String(value);
