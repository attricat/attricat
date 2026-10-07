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

export type ValueEditorProps = {
  attribute: Attribute;
  value: string;
  disabled: boolean;
  required?: boolean;
  error?: string;
  helperText?: string;
  onChange: (value: string) => void;
};

export type HeadingRenderer = ComponentType<{
  attributes: readonly Attribute[];
  /** Renders a smaller section heading for a panel beside another page. */
  compact?: boolean;
  entityId: string;
  values: Record<string, { value: unknown }>;
  view?: ViewDefinition;
}>;

export type IncomingRelationshipRenderer = ComponentType<{
  entityId: string;
  node: Extract<ViewNode, { type: 'incoming_relationship_list' }>;
}>;

/** Value types the generic field and table components display and edit. */
export const scalarValueTypes = [
  'string',
  'number',
  'integer',
  'boolean',
  'date',
  'datetime',
  'time',
] as const satisfies readonly Attribute['value_type'][];

export type ViewComponentDefinition = {
  id: string;
  version: number;
  capabilities: readonly Capability[];
  placements: readonly Placement[];
  value_types: readonly Attribute['value_type'][];
  allowed_props: readonly string[];
  valueRenderer?: ValueRenderer;
  valueEditor?: ComponentType<ValueEditorProps>;
  /** The edit component that replaces this display component while editing. */
  editComponentId?: string;
  /**
   * Keeps this display above the field's editor, for displays that show more
   * than the editor does, such as a relationship's ancestors.
   */
  showsWhileEditing?: boolean;
  validateValue?: (value: string) => string | undefined;
  /** Submit the edited string verbatim instead of trimming it. */
  preservesWhitespace?: boolean;
  /**
   * On the entity page, shows a set value with its display component until
   * the user asks to edit it, for values whose source reads poorly.
   */
  editsOnRequest?: boolean;
  headingRenderer?: HeadingRenderer;
  incomingRelationshipRenderer?: IncomingRelationshipRenderer;
};
