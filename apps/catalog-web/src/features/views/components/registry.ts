import type { ComponentReference } from '../../entities/api';
import { fieldDisplayComponent } from './FieldDisplay';
import { fieldEditComponent } from './FieldEdit';
import { relationshipListDisplayComponent } from './RelationshipListDisplay';
import { relationshipListEditComponent } from './RelationshipListEdit';
import { incomingRelationshipListDisplayComponent } from './IncomingRelationshipListDisplay';
import { tableDisplayComponent } from './TableDisplay';
import { tableEditComponent } from './TableEdit';
import { entityHeadingComponent } from './blocks/EntityHeadingConfig';
import type {
  HeadingRenderer,
  IncomingRelationshipRenderer,
  ValueRenderer,
  ViewComponentDefinition,
} from './component-types';

export const viewComponents = [
  fieldDisplayComponent,
  fieldEditComponent,
  relationshipListDisplayComponent,
  relationshipListEditComponent,
  incomingRelationshipListDisplayComponent,
  tableDisplayComponent,
  tableEditComponent,
  entityHeadingComponent,
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

export const resolveHeadingRenderer = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
): HeadingRenderer | undefined =>
  resolveViewComponent(component)?.headingRenderer;

export const resolveIncomingRelationshipRenderer = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
): IncomingRelationshipRenderer | undefined =>
  resolveViewComponent(component)?.incomingRelationshipRenderer;
