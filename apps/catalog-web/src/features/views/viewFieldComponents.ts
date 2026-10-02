import type {
  Attribute,
  ComponentReference,
  ViewDefinition,
  ViewNode,
} from '../entities/api';
import { resolveValueEditor } from './components/registry';

/** Retain configured field editors even when the form shows all attributes. */
export const viewFieldComponents = (view?: ViewDefinition) => {
  const result = new Map<string, ComponentReference>();
  const visit = (node: ViewNode | ViewDefinition) => {
    if (node.type === 'field' && node.component)
      result.set(node.field, node.component);
    if ('children' in node) node.children.forEach(visit);
    if (node.type === 'tabs')
      node.tabs.forEach((tab) => tab.children.forEach(visit));
    if (node.type === 'accordion')
      node.sections.forEach((section) => section.children.forEach(visit));
  };
  if (view) visit(view);
  return result;
};

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
