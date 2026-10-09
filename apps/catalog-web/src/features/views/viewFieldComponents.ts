import type {
  Attribute,
  ComponentReference,
  ViewDefinition,
  ViewNode,
} from '../entities/api';
import {
  resolveEditComponent,
  resolveValueEditor,
} from './components/registry';

const viewNodes = (view?: ViewDefinition | ViewNode) => {
  const nodes: (ViewNode | ViewDefinition)[] = [];
  const visit = (node: ViewNode | ViewDefinition) => {
    nodes.push(node);
    if ('children' in node) node.children.forEach(visit);
    if (node.type === 'tabs')
      node.tabs.forEach((tab) => tab.children.forEach(visit));
    if (node.type === 'accordion')
      node.sections.forEach((section) => section.children.forEach(visit));
  };
  if (view) visit(view);
  return nodes;
};

/** Retain configured field editors even when the form shows all attributes. */
export const viewFieldComponents = (view?: ViewDefinition) => {
  const result = new Map<string, ComponentReference>();
  for (const node of viewNodes(view))
    if (node.type === 'field' && node.component)
      result.set(node.field, node.component);
  return result;
};

/**
 * The component that edits each field a display view places: its paired edit
 * component, keyed by attribute code. Fields without one use the built-in
 * editor for their value type.
 */
export const viewFieldEditComponents = (view?: ViewDefinition) =>
  new Map(
    [...viewFieldComponents(view)].flatMap(([code, component]) => {
      const editor = resolveEditComponent(component);
      return editor ? [[code, editor] as const] : [];
    }),
  );

const placedFields = (view?: ViewDefinition | ViewNode) =>
  viewNodes(view).flatMap((node) =>
    node.type === 'field' || node.type === 'relationship_list'
      ? [node.field]
      : [],
  );

/**
 * Attribute codes placed inside blocks rendered by `componentId`, such as the
 * entity heading, in layout order.
 */
export const componentPlacedFields = (
  view: ViewDefinition | undefined,
  componentId: string,
) => [
  ...new Set(
    viewNodes(view)
      .filter(
        (node) => 'component' in node && node.component?.id === componentId,
      )
      .flatMap((node) => placedFields(node)),
  ),
];

/** Attribute codes placed anywhere within these nodes. */
export const nodesPlacedFields = (nodes: readonly ViewNode[]) =>
  new Set(nodes.flatMap((node) => placedFields(node)));

/** Attribute codes the view places as fields or relationship lists. */
export const viewPlacedFields = (view?: ViewDefinition) =>
  new Set(placedFields(view));

/** Edit components that apply to these attributes, keyed by attribute code. */
export const viewFieldEditors = (
  components: ReadonlyMap<string, ComponentReference>,
  attributes: readonly Attribute[],
) =>
  new Map(
    attributes.flatMap((attribute) => {
      const editor = resolveValueEditor(
        components.get(attribute.code),
        attribute,
      );
      return editor ? [[attribute.code, editor] as const] : [];
    }),
  );
