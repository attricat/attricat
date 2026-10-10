// Mirrors `crates/lexicon/src/reference.rs`; both implementations run the
// shared cases in `contracts/lexicon-references.json`.

export type LexiconReference = { key: string; context?: string };
export type LexiconSegment = { literal: string } | LexiconReference;
export type LexiconReferenceError =
  'unclosed' | 'empty_key' | 'empty_context' | 'multiple_contexts' | 'brace';
export type LexiconParseResult =
  { segments: LexiconSegment[] } | { error: LexiconReferenceError };

const open = '{{';
const close = '}}';
const escape = '\\';
const contextSeparator = '|';

/** Trims and collapses whitespace, the comparison form of keys and contexts. */
export const normalizeLexiconTerm = (text: string) =>
  text.trim().replace(/\s+/gu, ' ');

const parseReference = (
  body: string,
): LexiconReference | LexiconReferenceError => {
  if (body.includes('{') || body.includes('}')) return 'brace';
  const parts = body.split(contextSeparator);
  if (parts.length > 2) return 'multiple_contexts';
  const key = normalizeLexiconTerm(parts[0]);
  if (!key) return 'empty_key';
  if (parts.length === 1) return { key };
  const context = normalizeLexiconTerm(parts[1]);
  return context ? { key, context } : 'empty_context';
};

/** Splits catalog text into literal runs and `{{key|context}}` references. */
export const parseLexiconText = (text: string): LexiconParseResult => {
  const segments: LexiconSegment[] = [];
  let literal = '';
  let rest = text;
  for (let start = rest.indexOf(open); start >= 0; start = rest.indexOf(open)) {
    if (rest.slice(0, start).endsWith(escape)) {
      literal += rest.slice(0, start - escape.length) + open;
      rest = rest.slice(start + open.length);
      continue;
    }
    literal += rest.slice(0, start);
    const body = rest.slice(start + open.length);
    const end = body.indexOf(close);
    if (end < 0) return { error: 'unclosed' };
    const reference = parseReference(body.slice(0, end));
    if (typeof reference === 'string') return { error: reference };
    if (literal) segments.push({ literal });
    literal = '';
    segments.push(reference);
    rest = body.slice(end + close.length);
  }
  literal += rest;
  if (literal) segments.push({ literal });
  return { segments };
};

export const isLexiconReference = (
  segment: LexiconSegment,
): segment is LexiconReference => 'key' in segment;
