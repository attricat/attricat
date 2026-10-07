import type { Attribute, ComponentReference } from '../../entities/api';
import { stringControlComponents } from '../controls/definitions';
import { fieldDisplayComponent } from './FieldDisplay';
import { fieldEditComponent } from './FieldEdit';
import { relationshipListDisplayComponent } from './RelationshipListDisplay';
import { relationshipListEditComponent } from './RelationshipListEdit';
import { incomingRelationshipListDisplayComponent } from './IncomingRelationshipListDisplay';
import { tableDisplayComponent } from './TableDisplay';
import { tableImageComponent } from './TableImage';
import { tableEditComponent } from './TableEdit';
import { entityHeadingComponent } from './blocks/EntityHeadingConfig';
import { relationshipHierarchyComponent } from './RelationshipHierarchy';
import type {
  HeadingRenderer,
  IncomingRelationshipRenderer,
  ValueRenderer,
  ViewComponentDefinition,
} from './componentTypes';

export const viewComponents: readonly ViewComponentDefinition[] = [
  ...stringControlComponents,
  fieldDisplayComponent,
  fieldEditComponent,
  relationshipListDisplayComponent,
  relationshipListEditComponent,
  incomingRelationshipListDisplayComponent,
  tableDisplayComponent,
  tableImageComponent,
  tableEditComponent,
  entityHeadingComponent,
  relationshipHierarchyComponent,
] satisfies readonly ViewComponentDefinition[];

export type RegisteredViewComponent = (typeof viewComponents)[number];

const componentKey = (id: string, version: number) => `${id}@${version}`;

/** A blueprint component reference, or nothing when none is configured. */
type ComponentSelector =
  Pick<ComponentReference, 'id' | 'version'> | null | undefined;

export const viewComponentRegistry = new Map<string, RegisteredViewComponent>(
  viewComponents.map((component) => [
    componentKey(component.id, component.version),
    component,
  ]),
);

export const resolveViewComponent = (component: ComponentSelector) =>
  component
    ? viewComponentRegistry.get(componentKey(component.id, component.version))
    : undefined;

export const resolveValueRenderer = (
  component: ComponentSelector,
): ValueRenderer | undefined => resolveViewComponent(component)?.valueRenderer;

/** A field's configured edit component, if it supports the attribute's value type. */
export const resolveValueEditor = (
  component: ComponentSelector,
  attribute: Pick<Attribute, 'value_type'>,
): RegisteredViewComponent | undefined => {
  const definition = resolveViewComponent(component);
  return definition?.valueEditor &&
    definition.value_types.includes(attribute.value_type)
    ? definition
    : undefined;
};

/**
 * The component that edits a field placed with `component`: an edit component
 * as is, or the edit counterpart of a display component.
 */
export const resolveEditComponent = (
  component: ComponentSelector,
): ComponentReference | undefined => {
  const definition = resolveViewComponent(component);
  if (!component || !definition) return undefined;
  if (definition.valueEditor) return { props: {}, ...component };
  const editor = definition.editComponentId
    ? viewComponentRegistry.get(
        componentKey(definition.editComponentId, definition.version),
      )
    : undefined;
  return editor
    ? { id: editor.id, version: editor.version, props: {} }
    : undefined;
};

export const resolveHeadingRenderer = (
  component: ComponentSelector,
): HeadingRenderer | undefined =>
  resolveViewComponent(component)?.headingRenderer;

export const resolveIncomingRelationshipRenderer = (
  component: ComponentSelector,
): IncomingRelationshipRenderer | undefined =>
  resolveViewComponent(component)?.incomingRelationshipRenderer;
