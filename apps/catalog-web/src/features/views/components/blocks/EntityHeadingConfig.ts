import type { ViewComponentDefinition } from '../componentTypes';
import { EntityHeading } from './EntityHeading';
import { entityHeadingComponentId } from './EntityHeadingDefinition';

export const entityHeadingComponent = {
  id: entityHeadingComponentId,
  version: 1,
  capabilities: ['display'],
  placements: ['stack'],
  value_types: [],
  allowed_props: [],
  headingRenderer: EntityHeading,
} satisfies ViewComponentDefinition;
