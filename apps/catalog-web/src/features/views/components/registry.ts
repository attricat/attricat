import type { ComponentReference } from '../../entities/api';
import { colorDisplayComponent, colorEditComponent } from './colorComponents';
import { emailDisplayComponent, emailEditComponent } from './emailComponents';
import { urlDisplayComponent, urlEditComponent } from './urlComponents';
import { fieldDisplayComponent } from './FieldDisplay';
import { fieldEditComponent } from './FieldEdit';
import { phoneDisplayComponent, phoneEditComponent } from './phone';
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
  colorDisplayComponent,
  colorEditComponent,
  emailDisplayComponent,
  emailEditComponent,
  urlDisplayComponent,
  urlEditComponent,
  phoneDisplayComponent,
  phoneEditComponent,
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

export const resolveHeadingRenderer = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
): HeadingRenderer | undefined =>
  resolveViewComponent(component)?.headingRenderer;

export const resolveIncomingRelationshipRenderer = (
  component: Pick<ComponentReference, 'id' | 'version'> | null | undefined,
): IncomingRelationshipRenderer | undefined =>
  resolveViewComponent(component)?.incomingRelationshipRenderer;
