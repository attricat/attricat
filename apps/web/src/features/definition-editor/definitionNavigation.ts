import {
  scanYaml,
  type TextRange,
  type YamlOutline,
} from '../../components/yamlOutline';
import {
  attributePathSeparator,
  attributesKey,
  codeKey,
  viewsKey,
} from './constants';

/** A place in a definition to show: what to select, inside which block. */
export type DefinitionTarget = {
  /** The block that declares it, highlighted around the selection. */
  block: TextRange;
  /** The name to select, such as an attribute's code. */
  focus: TextRange;
};

/** Where the definition declares the attribute with `code`. */
export const attributeDefinition = (
  text: string,
  code: string,
): DefinitionTarget | undefined => {
  const outline = scanYaml(text);
  const declared = outline.entries.find(
    ({ path, value }) =>
      path.length === 3 &&
      path[0] === attributesKey &&
      path[2] === codeKey &&
      value === code,
  );
  const focus = declared?.valueRange;
  if (!declared || !focus) return undefined;
  const index = declared.path[1];
  const block =
    outline.collections.find(
      ({ path }) =>
        path.length === 2 && path[0] === attributesKey && path[1] === index,
    )?.range ?? focus;
  return { block, focus };
};

const viewTarget = (
  outline: YamlOutline,
  name: string,
): DefinitionTarget | undefined => {
  const focus = outline.entryAt([viewsKey, name])?.range;
  if (!focus) return undefined;
  const body = outline.collections.find(
    ({ path }) => path.length === 2 && path[0] === viewsKey && path[1] === name,
  )?.range;
  return {
    block: body ? { end: body.end, start: focus.start } : focus,
    focus,
  };
};

/** Where the definition declares the view called `name`. */
export const viewDefinition = (text: string, name: string) =>
  viewTarget(scanYaml(text), name);

export const definitionSectionKinds = {
  attribute: 'attribute',
  view: 'view',
} as const;

/** The part of a definition that the editor's cursor is in. */
export type DefinitionSection =
  | { code: string; kind: typeof definitionSectionKinds.attribute }
  | {
      /** The attribute named by the field reference under the cursor. */
      field?: string;
      kind: typeof definitionSectionKinds.view;
      name: string;
    };

/** Keys of view blocks and columns whose values name an attribute. */
const fieldKey = 'field';
const fieldsKey = 'fields';

const contains = (range: TextRange, offset: number) =>
  offset >= range.start && offset <= range.end;

/** The attribute or view declared around `offset`, if any. */
export const definitionSectionAt = (
  text: string,
  offset: number,
): DefinitionSection | undefined => {
  const outline = scanYaml(text);
  const attribute = outline.collections.find(
    ({ path, range }) =>
      path.length === 2 && path[0] === attributesKey && contains(range, offset),
  );
  if (attribute) {
    const code = outline.valueAt([...attribute.path, codeKey]);
    return typeof code === 'string'
      ? { code, kind: definitionSectionKinds.attribute }
      : undefined;
  }
  const view = outline.entries.find(({ path }) => {
    if (path.length !== 2 || path[0] !== viewsKey) return false;
    const target = viewTarget(outline, String(path[1]));
    return target !== undefined && contains(target.block, offset);
  });
  if (!view) return undefined;
  const reference = outline.entries.find(
    ({ path, value, valueRange }) =>
      path[0] === viewsKey &&
      path[1] === view.path[1] &&
      typeof value === 'string' &&
      valueRange !== undefined &&
      contains(valueRange, offset) &&
      (path.at(-1) === fieldKey || path.at(-2) === fieldsKey),
  );
  return {
    field:
      typeof reference?.value === 'string'
        ? reference.value.split(attributePathSeparator)[0]
        : undefined,
    kind: definitionSectionKinds.view,
    name: String(view.path[1]),
  };
};
