import { viewBlockTypes, type ViewDefinition, type ViewNode } from './schemas';

/**
 * A `detail` stack without children only lists the extensions placed on the
 * record page; the page keeps its default layout.
 */
export const placesOnlyExtensions = (view: ViewDefinition | undefined) =>
  view?.type === viewBlockTypes.stack && view.children.length === 0;

/** A view that lays out a record's fields, not a table, option or the like. */
export const isLayoutView = (
  view: ViewDefinition | undefined,
): view is ViewDefinition & ViewNode =>
  !(
    !view ||
    view.type === viewBlockTypes.table ||
    view.type === viewBlockTypes.dropdownOption ||
    view.type === viewBlockTypes.pageTitle ||
    view.type === viewBlockTypes.explorer ||
    placesOnlyExtensions(view)
  );
