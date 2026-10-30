import i18n from '../../i18n';
import type { TextRange, YamlOutline } from '../../components/yamlOutline';
import {
  aliasKey,
  attributePathSeparator,
  attributesKey,
  codeKey,
  codePattern,
  includesKey,
  type DefinitionKind,
} from './constants';
import {
  referenceAt,
  referenceDeclaration,
  referenceValues,
} from './definitionReferenceTargets';

/** An attribute code or include alias that the definition declares. */
export type DeclaredName = {
  /** Where the name is declared, without quotes. */
  declaration: TextRange;
  name: string;
  /** The table that declares it: `attributes` or `includes`. */
  table: string;
};

const quotes = new Set(['"', "'"]);

/** The leading `length` characters of a scalar, skipping an opening quote. */
const nameRange = (text: string, range: TextRange, length: number) => {
  const start = range.start + (quotes.has(text[range.start]) ? 1 : 0);
  return { end: start + length, start };
};

const contains = (range: TextRange, offset: number) =>
  offset >= range.start && offset <= range.end;

/** Tables whose items declare a name, and the key that holds it. */
const declaringKeys: Record<string, string> = {
  [attributesKey]: codeKey,
  [includesKey]: aliasKey,
};

const declarationAt = (
  text: string,
  outline: YamlOutline,
  offset: number,
): DeclaredName | undefined => {
  const entry = outline.entries.find(
    ({ path, value, valueRange }) =>
      path.length === 3 &&
      declaringKeys[String(path[0])] === path[2] &&
      typeof value === 'string' &&
      valueRange !== undefined &&
      contains(valueRange, offset),
  );
  if (!entry?.valueRange || typeof entry.value !== 'string') return undefined;
  return {
    declaration: nameRange(text, entry.valueRange, entry.value.length),
    name: entry.value,
    table: String(entry.path[0]),
  };
};

/**
 * The declared name at `offset`: on its declaration, or on the first
 * segment of a reference to it, such as `category` in `category.name`.
 */
export const declaredNameAt = (
  kind: DefinitionKind,
  text: string,
  outline: YamlOutline,
  offset: number,
): DeclaredName | undefined => {
  const declared = declarationAt(text, outline, offset);
  if (declared) return declared;
  const target = referenceAt(kind, outline, offset);
  const declaration = target && referenceDeclaration(outline, target);
  if (!target || !declaration) return undefined;
  const [name] = target.value.split(attributePathSeparator);
  if (!contains(nameRange(text, target.range, name.length), offset))
    return undefined;
  return declarationAt(text, outline, declaration.start);
};

/** Where `declared` is declared and every reference to it. */
export const nameUsages = (
  kind: DefinitionKind,
  text: string,
  outline: YamlOutline,
  { declaration, name }: DeclaredName,
): TextRange[] => [
  declaration,
  ...referenceValues(kind, outline).flatMap((target) => {
    const declared = referenceDeclaration(outline, target);
    return declared &&
      nameRange(text, declared, name.length).start === declaration.start
      ? [nameRange(text, target.range, name.length)]
      : [];
  }),
];

const validCode = new RegExp(codePattern);

/** Why `name` cannot replace a declared name, if it cannot. */
export const renameProblem = (name: string) =>
  validCode.test(name)
    ? undefined
    : i18n.t('definitionEditor.diagnostics.invalidCode');
