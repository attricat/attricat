import { Box, Chip } from '@mui/material';
import type { ReactNode } from 'react';

export const RelationshipSelectionPills = ({
  action,
  ids,
  labels,
  onRemove,
}: {
  action?: ReactNode;
  ids: string[];
  labels: Map<string, string>;
  onRemove?: (id: string) => void;
}) => {
  if (ids.length === 0 && !action) return null;
  return (
    <Box
      sx={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: 2,
        minWidth: 0,
      }}
    >
      {ids.map((id) => (
        <Chip
          key={id}
          label={labels.get(id) ?? id}
          onDelete={onRemove ? () => onRemove(id) : undefined}
          size="small"
          sx={{ maxWidth: '100%' }}
        />
      ))}
      {action}
    </Box>
  );
};
