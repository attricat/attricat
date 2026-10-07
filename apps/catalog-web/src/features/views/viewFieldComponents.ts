import type {
  Attribute,
  ComponentReference,
  ViewDefinition,
  ViewNode,
} from '../entities/api';
import { resolveValueEditor } from './components/registry';

const viewNodes = (view?: ViewDefinition, skipComponentId?: string) => {
  const nodes: (ViewNode | ViewDefinition)[] = [];
  const visit = (node: ViewNode | ViewDefinition) => {
    if (
      skipComponentId &&
      'component' in node &&
      node.component?.id === skipComponentId
    )
      return;
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
 * Attribute codes the view places as fields or relationship lists, ignoring
 * the subtree of a component the caller renders elsewhere.
 */
export const viewPlacedFields = (
  view?: ViewDefinition,
  skipComponentId?: string,
) =>
  new Set(
    viewNodes(view, skipComponentId).flatMap((node) =>
      node.type === 'field' || node.type === 'relationship_list'
        ? [node.field]
        : [],
    ),
  );

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
