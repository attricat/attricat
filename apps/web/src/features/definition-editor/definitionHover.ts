import i18n from '../../i18n';
import type { DefinitionKind } from './constants';
import {
  choicesOf,
  definitionSchemas,
  schemaAtPath,
  typeLabel,
} from './definitionSchema';
import { scanToml, type TextRange } from './tomlOutline';

export type DefinitionHover = { contents: string[]; range: TextRange };

const contains = (range: TextRange, offset: number) =>
  offset >= range.start && offset <= range.end;

/** Documents the key or table header under the cursor from its contract. */
export const definitionHover = (
  kind: DefinitionKind,
  text: string,
  offset: number,
): DefinitionHover | undefined => {
  const root = definitionSchemas[kind];
  const outline = scanToml(text);
  const target =
    outline.entries.find(({ range }) => contains(range, offset)) ??
    outline.headers.find(({ range }) => contains(range, offset));
  if (!target) return undefined;
  const { path } = target;
  // Elements and `[[array]]` headers end in an index; describe the array.
  const described = typeof path.at(-1) === 'number' ? path.slice(0, -1) : path;
  const node = schemaAtPath(root, described, outline.valueAt)?.node;
  if (!node) return undefined;
  const name = described
    .filter((segment) => typeof segment === 'string')
    .at(-1);
  const choices = choicesOf(node);
  const contents = [
    `**${name ?? ''}** \`${typeLabel(root, node)}\``,
    node.description,
    choices &&
      i18n.t('definitionEditor.hover.allowedValues', {
        values: choices.map((choice) => `\`${choice}\``).join(', '),
      }),
    node.default !== undefined &&
      node.default !== null &&
      i18n.t('definitionEditor.hover.default', {
        value: `\`${JSON.stringify(node.default)}\``,
      }),
  ].filter((content): content is string => Boolean(content));
  return { contents, range: target.range };
};
