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

export const viewComponentRegistry = new Map<string, RegisteredViewComponent>(
  viewComponents.map((component) => [
    componentKey(component.id, component.version),
    component,
  ]),
);

export const resolveViewComponent = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
) =>
  component
    ? viewComponentRegistry.get(componentKey(component.id, component.version))
    : undefined;

export const resolveValueRenderer = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
): ValueRenderer | undefined => resolveViewComponent(component)?.valueRenderer;

/** A field's configured edit component, if it supports the attribute's value type. */
export const resolveValueEditor = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
  attribute: Pick<Attribute, 'value_type'>,
): RegisteredViewComponent | undefined => {
  const definition = resolveViewComponent(component);
  return definition?.valueEditor &&
    definition.value_types.includes(attribute.value_type)
    ? definition
    : undefined;
};

export const resolveHeadingRenderer = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
): HeadingRenderer | undefined =>
  resolveViewComponent(component)?.headingRenderer;

export const resolveIncomingRelationshipRenderer = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
): IncomingRelationshipRenderer | undefined =>
  resolveViewComponent(component)?.incomingRelationshipRenderer;
