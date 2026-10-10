import type { ReusableAttribute } from './api';

export const latestReusableAttributeRevisions = (
  attributes: ReusableAttribute[],
): ReusableAttribute[] => {
  const latest = new Map<string, ReusableAttribute>();
  for (const attribute of attributes) {
    const current = latest.get(attribute.definition_id);
    if (!current || attribute.version > current.version) {
      latest.set(attribute.definition_id, attribute);
    }
  }
  return [...latest.values()];
};
