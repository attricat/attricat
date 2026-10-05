import { Box } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { SHORT_ENTITY_ID_LENGTH } from './constants';
import { ruleRevisionKey, type RuleRevisions } from './ruleRevisions';

export const RuleRevisionCell = ({
  revisions,
  ruleId,
  version,
}: {
  revisions: RuleRevisions;
  ruleId: string;
  version: number;
}) => {
  const { t } = useTranslation();
  const rule = revisions.get(ruleRevisionKey(ruleId, version));
  return rule
    ? t('rules.ruleRevision', { name: rule.name, version })
    : t('rules.unknownRule', {
        id: ruleId.slice(0, SHORT_ENTITY_ID_LENGTH),
        version,
      });
};

// Findings and runs carry only entity IDs, and there is no batch label lookup.
export const EntityIdLink = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  return (
    <Link
      aria-label={t('rules.openEntity', { id: entityId })}
      params={{ entityId }}
      title={entityId}
      to="/entities/$entityId"
    >
      <Box component="span" sx={{ fontFamily: 'monospace' }}>
        {entityId.slice(0, SHORT_ENTITY_ID_LENGTH)}
      </Box>
    </Link>
  );
};
