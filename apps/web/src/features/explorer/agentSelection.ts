import type { TFunction } from 'i18next';
import type { RecordItem } from '../records/api';
import { displayLabel } from '../records/recordDisplay';
import { maximumAgentRecordLabelLength } from './constants';

export { maximumAgentSelection } from './constants';

export const selectedRecordsMessage = (
  t: TFunction,
  instructions: string,
  blueprintName: string,
  records: RecordItem[],
) => {
  const references = records
    .map((record) =>
      t('explorer.agentMessage.recordReference', {
        recordId: record.id,
        label: displayLabel(record.display, record.id)
          .replaceAll('\n', ' ')
          .slice(0, maximumAgentRecordLabelLength),
      }),
    )
    .join('\n');
  return [
    instructions.trim(),
    '',
    t('explorer.agentMessage.blueprint', { blueprint: blueprintName }),
    t('explorer.agentMessage.selectedRecords'),
    references,
    '',
    t('explorer.agentMessage.getRecordInstruction'),
  ].join('\n');
};
