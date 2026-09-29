import type { ComponentType, ReactNode } from 'react';
import type {
  Attribute,
  ComponentReference,
  ViewDefinition,
  ViewNode,
} from '../../entities/api';

type Capability = 'display' | 'edit';
type Placement =
  | 'field'
  | 'relationship_list'
  | 'incoming_relationship_list'
  | 'table'
  | 'stack';

export type ValueRenderer = ComponentType<{
  attribute: Attribute;
  component?: ComponentReference | null;
  contextId?: string;
  entityId?: string;
  value: unknown;
  renderFilePanel?: (fileId: string) => ReactNode;
}>;

export type HeadingRenderer = ComponentType<{
  attributes: readonly Attribute[];
  entityId: string;
  values: Record<string, { value: unknown }>;
  view?: ViewDefinition;
}>;

export type IncomingRelationshipRenderer = ComponentType<{
  entityId: string;
  node: Extract<ViewNode, { type: 'incoming_relationship_list' }>;
}>;

export type ViewComponentDefinition = {
  id: string;
  version: number;
  capabilities: readonly Capability[];
  placements: readonly Placement[];
  value_types: readonly Attribute['value_type'][];
  allowed_props: readonly string[];
  valueRenderer?: ValueRenderer;
  headingRenderer?: HeadingRenderer;
  incomingRelationshipRenderer?: IncomingRelationshipRenderer;
};
