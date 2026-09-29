import type { TFunction } from 'i18next';
import type { EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entityDisplay';
import { maximumAgentEntityLabelLength } from './constants';

export { maximumAgentSelection } from './constants';

export const selectedEntitiesMessage = (
  t: TFunction,
  instructions: string,
  blueprintName: string,
  entities: EntityItem[],
) => {
  const references = entities
    .map((entity) =>
      t('explorer.agentMessage.entityReference', {
        entityId: entity.id,
        label: displayLabel(entity.display, entity.id)
          .replaceAll('\n', ' ')
          .slice(0, maximumAgentEntityLabelLength),
      }),
    )
    .join('\n');
  return [
    instructions.trim(),
    '',
    t('explorer.agentMessage.blueprint', { blueprint: blueprintName }),
    t('explorer.agentMessage.selectedEntities'),
    references,
    '',
    t('explorer.agentMessage.getEntityInstruction'),
  ].join('\n');
};
