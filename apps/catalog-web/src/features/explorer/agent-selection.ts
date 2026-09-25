import type { EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entity-display';

export const maximumAgentSelection = 50;

export const selectedEntitiesMessage = (
  instructions: string,
  blueprintName: string,
  entities: EntityItem[],
) =>
  `${instructions.trim()}\n\nBlueprint: ${blueprintName}\nSelected entities (IDs are authoritative; labels are for reference):\n${entities
    .map(
      (entity) =>
        `- ${displayLabel(entity.display, entity.id).replaceAll('\n', ' ').slice(0, 60)} — entity_id: ${entity.id}`,
    )
    .join(
      '\n',
    )}\n\nUse get_entity for each entity_id before making recommendations or changes.`;
