import type { ViewDefinition, ViewNode } from '../entities/api';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from './constants';

/** Navigation policy, independent of editor and server-side schema validation. */
export const safeUrl = (value: unknown): string | undefined => {
  if (typeof value !== 'string' || !/^https?:\/\/[^/?#]/i.test(value))
    return undefined;
  // Do not let URL's permissive parser silently strip whitespace or reinterpret backslashes.
  if (/[\s\p{Cc}\\]/u.test(value)) return undefined;
  try {
    const url = new URL(value);
    if (!url.hostname || url.username || url.password) return undefined;
    return url.href;
  } catch {
    return undefined;
  }
};

export const urlEditFields = (view?: ViewDefinition): Set<string> => {
  const fields = new Set<string>();
  const visit = (node: ViewNode | ViewDefinition) => {
    if (
      node.type === 'field' &&
      node.component?.id === VIEW_COMPONENT_IDS.urlEdit &&
      node.component.version === VIEW_COMPONENT_VERSION
    )
      fields.add(node.field);
    if ('children' in node) node.children.forEach(visit);
    if ('tabs' in node) node.tabs.forEach((tab) => tab.children.forEach(visit));
    if ('sections' in node)
      node.sections.forEach((section) => section.children.forEach(visit));
  };
  if (view) visit(view);
  return fields;
};
