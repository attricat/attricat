import type { ViewComponentDefinition } from '../componentTypes';
import { RecordHeading } from './RecordHeading';
import { recordHeadingComponentId } from './RecordHeadingDefinition';
import { VIEW_COMPONENT_VERSION } from '../../constants';

export const recordHeadingComponent = {
  id: recordHeadingComponentId,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['stack'],
  value_types: [],
  allowed_props: [],
  headingRenderer: RecordHeading,
} satisfies ViewComponentDefinition;
