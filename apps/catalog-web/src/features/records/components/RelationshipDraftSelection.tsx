import { Box, Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';

type Props = {
  ids: readonly string[];
  labelForId: (id: string) => string;
  onRemove: (id: string) => void;
};

/** Targets chosen in the relationship selector before they are applied. */
export const RelationshipDraftSelection = ({
  ids,
  labelForId,
  onRemove,
}: Props) => {
  const { t } = useTranslation();
  if (ids.length === 0) return null;
  return (
    <Stack spacing={0.5}>
      <Typography variant="subtitle2">
        {t('records.selectedRelationships')}
      </Typography>
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
        {ids.map((id) => (
          <Chip
            color="primary"
            key={id}
            label={labelForId(id)}
            onDelete={() => onRemove(id)}
            size="small"
            variant="outlined"
          />
        ))}
      </Box>
    </Stack>
  );
};
