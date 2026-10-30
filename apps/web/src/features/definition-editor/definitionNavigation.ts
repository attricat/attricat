import { scanYaml, type TextRange } from '../../components/yamlOutline';
import { attributesKey, codeKey, viewsKey } from './constants';

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

/** Where the definition declares the view called `name`. */
export const viewDefinition = (
  text: string,
  name: string,
): DefinitionTarget | undefined => {
  const outline = scanYaml(text);
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
