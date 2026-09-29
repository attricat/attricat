import { Paper, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { summaryCardMinWidth } from './constants';
import type { DataHealthSummary } from './schemas';

export const DataHealthSummaryCards = ({
  staleAfterDays,
  summary,
}: {
  staleAfterDays: number;
  summary: DataHealthSummary;
}) => {
  const { t } = useTranslation();
  const cards = [
    [t('dataHealth.outdatedEntities'), summary.outdated_entities],
    [t('dataHealth.activeEntities'), summary.active_entities],
    [
      t('dataHealth.staleAfterDays', { count: staleAfterDays }),
      summary.stale_entities,
    ],
    [
      t('dataHealth.deletedRelationshipTargets'),
      summary.deleted_relationship_targets,
    ],
  ];

  return (
    <Stack
      direction={{ xs: 'column', sm: 'row' }}
      spacing={2}
      sx={{ flexWrap: 'wrap', mt: 4 }}
    >
      {cards.map(([label, value]) => (
        <Paper key={String(label)} sx={{ minWidth: summaryCardMinWidth, p: 2 }}>
          <Typography color="text.secondary" variant="body2">
            {label}
          </Typography>
          <Typography variant="h4">{value}</Typography>
        </Paper>
      ))}
    </Stack>
  );
};
