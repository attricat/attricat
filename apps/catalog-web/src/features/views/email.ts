import type {
  ComponentReference,
  ViewDefinition,
  ViewNode,
} from '../entities/api';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from './constants';

// A single ASCII dot-atom mailbox. No display names, recipient lists or URI headers.
const emailPattern =
  /^[A-Za-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[A-Za-z0-9!#$%&'*+/=?^_`{|}~-]+)*@(?:[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?\.)*[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$/;
const maximumEmailLength = 254;
const maximumLocalLength = 64;
const maximumDomainLabelLength = 63;

export const isEmailAddress = (value: string) => {
  const [local, domain] = value.split('@');
  return (
    value === value.trim() &&
    value.length <= maximumEmailLength &&
    local.length <= maximumLocalLength &&
    emailPattern.test(value) &&
    domain.split('.').every((label) => label.length <= maximumDomainLabelLength)
  );
};

export const emailHref = (value: unknown): string | undefined =>
  typeof value === 'string' && isEmailAddress(value)
    ? `mailto:${encodeURIComponent(value).replace(/%40/g, '@')}`
    : undefined;

export const isEmailEditor = (component?: ComponentReference | null) =>
  component?.id === VIEW_COMPONENT_IDS.emailEdit &&
  component.version === VIEW_COMPONENT_VERSION;

export const emailFieldsInView = (view?: ViewDefinition): Set<string> => {
  const fields = new Set<string>();
  const visit = (node: ViewDefinition | ViewNode) => {
    if (node.type === 'field' && isEmailEditor(node.component))
      fields.add(node.field);
    if ('children' in node) node.children.forEach(visit);
    if ('tabs' in node) node.tabs.forEach((tab) => tab.children.forEach(visit));
    if ('sections' in node)
      node.sections.forEach((section) => section.children.forEach(visit));
  };
  if (view) visit(view);
  return fields;
};
