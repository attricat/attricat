import { z } from 'zod';
import contract from '../../../../../contracts/view-components.json';
import type { TomlPath } from './tomlOutline';

const viewComponentSchema = z.object({
  capabilities: z.array(z.string()),
  id: z.string(),
  placements: z.array(z.string()),
  value_types: z.array(z.string()),
  version: z.number().int().positive(),
});

/** Platform components that blueprint views may reference. */
export const viewComponents = z
  .object({ components: z.array(viewComponentSchema) })
  .parse(contract).components;

const viewsKey = 'views';
const rendererKey = 'renderer';
/** Column renderers are validated as display components of the table. */
const tablePlacement = 'table';
/** Mirrors `required_view_capability` in the Rust component manifest. */
const viewCapabilities: Record<string, string> = {
  detail: 'display',
  edit: 'edit',
  table: 'display',
};

/**
 * Where a component reference at `path` is placed: the owning block type (or
 * a table column), the view's required capability, and the rendered field.
 */
export const viewComponentPlacement = (
  path: TomlPath,
  valueAt: (path: TomlPath) => string | undefined,
) => {
  const view = path[0] === viewsKey ? path[1] : undefined;
  const owner = path.slice(0, -1);
  const column = path.at(-1) === rendererKey;
  return {
    capability: column
      ? viewCapabilities[tablePlacement]
      : typeof view === 'string'
        ? viewCapabilities[view]
        : undefined,
    field: valueAt([...owner, 'field']),
    placement: column ? tablePlacement : valueAt([...owner, 'type']),
  };
};
