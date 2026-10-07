import { Box, Chip } from '@mui/material';
import type { MouseEvent, ReactNode } from 'react';
import { RouterChip } from '../../../components/RouterLink';

export const RelationshipSelectionPills = ({
  action,
  ids,
  labels,
  linkToEntities = false,
  onRemove,
}: {
  action?: ReactNode;
  ids: string[];
  labels: Map<string, string>;
  /** Each pill opens its entity; removing one stays on the page. */
  linkToEntities?: boolean;
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
      {ids.map((id) =>
        linkToEntities ? (
          <RouterChip
            clickable
            key={id}
            label={labels.get(id) ?? id}
            onDelete={
              onRemove
                ? (event: MouseEvent) => {
                    // The delete icon sits inside the link.
                    event.preventDefault();
                    onRemove(id);
                  }
                : undefined
            }
            params={{ entityId: id }}
            size="small"
            sx={{ maxWidth: '100%' }}
            to="/entities/$entityId"
          />
        ) : (
          <Chip
            key={id}
            label={labels.get(id) ?? id}
            onDelete={onRemove ? () => onRemove(id) : undefined}
            size="small"
            sx={{ maxWidth: '100%' }}
          />
        ),
      )}
      {action}
    </Box>
  );
};
