import { Box, Chip } from '@mui/material';
import type { MouseEvent, ReactNode } from 'react';
import { RouterChip } from '../../../components/RouterLink';

export const RelationshipSelectionPills = ({
  action,
  gap = 2,
  ids,
  labels,
  linkToRecords = false,
  onRemove,
}: {
  action?: ReactNode;
  /** Spacing between pills, in theme units. */
  gap?: number;
  ids: string[];
  labels: Map<string, string>;
  /** Each pill opens its record; removing one stays on the page. */
  linkToRecords?: boolean;
  onRemove?: (id: string) => void;
}) => {
  if (ids.length === 0 && !action) return null;
  return (
    <Box
      sx={{
        display: 'flex',
        flexWrap: 'wrap',
        gap,
        minWidth: 0,
      }}
    >
      {ids.map((id) =>
        linkToRecords ? (
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
            params={{ recordId: id }}
            size="small"
            sx={{ maxWidth: '100%' }}
            to="/records/$recordId"
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
