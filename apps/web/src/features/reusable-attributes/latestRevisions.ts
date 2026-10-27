import { latestRevisions } from '../../api/revisionStatus';
import type { ReusableAttribute } from './api';

export const latestReusableAttributeRevisions = (
  attributes: ReusableAttribute[],
): ReusableAttribute[] =>
  latestRevisions(attributes, (attribute) => attribute.definition_id);
