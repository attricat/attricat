import type { EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entityDisplay';

export const maximumAgentSelection = 50;

export const selectedEntitiesMessage = (
  instructions: string,
  blueprintName: string,
  entities: EntityItem[],
) => {
  const references = entities
    .map(
      (entity) =>
        `- ${displayLabel(entity.display, entity.id).replaceAll('\n', ' ').slice(0, 60)} — entity_id: ${entity.id}`,
    )
    .join('\n');
  return `${instructions.trim()}\n\nBlueprint: ${blueprintName}\nSelected entities (IDs are authoritative; labels are for reference):\n${references}\n\nUse get_entity for each entity_id before making recommendations or changes.`;
};
