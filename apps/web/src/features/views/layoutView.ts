import { viewBlockTypes, type ViewDefinition, type ViewNode } from './schemas';

/** A view that lays out a record's fields, not a table, option or the like. */
export const isLayoutView = (
  view: ViewDefinition | undefined,
): view is ViewDefinition & ViewNode =>
  !(
    !view ||
    view.type === viewBlockTypes.table ||
    view.type === viewBlockTypes.dropdownOption ||
    view.type === viewBlockTypes.pageTitle ||
    view.type === viewBlockTypes.extensionLayout ||
    view.type === viewBlockTypes.explorer
  );
