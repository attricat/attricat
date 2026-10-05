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

/** Links an entity by its display label, or by a short ID until one loads. */
export const EntityIdLink = ({
  entityId,
  label,
}: {
  entityId: string;
  label: string | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <Link params={{ entityId }} title={entityId} to="/entities/$entityId">
      {label ?? (
        <Box
          aria-label={t('rules.entityWithoutLabel', { id: entityId })}
          component="span"
          sx={{ fontFamily: 'monospace' }}
        >
          {entityId.slice(0, SHORT_ENTITY_ID_LENGTH)}
        </Box>
      )}
    </Link>
  );
};
