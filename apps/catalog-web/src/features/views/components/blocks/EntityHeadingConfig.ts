import type { ViewComponentDefinition } from '../componentTypes';
import { EntityHeading } from './EntityHeading';
import { entityHeadingComponentId } from './EntityHeadingDefinition';
import { VIEW_COMPONENT_VERSION } from '../../constants';

export const entityHeadingComponent = {
  id: entityHeadingComponentId,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['stack'],
  value_types: [],
  allowed_props: [],
  headingRenderer: EntityHeading,
} satisfies ViewComponentDefinition;
