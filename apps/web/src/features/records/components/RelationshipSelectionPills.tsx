import { Box, Chip } from '@mui/material';
import type { MouseEvent, ReactNode } from 'react';
import { NewTabIcon } from '../../../components/NewTabIcon';
import { RouterChip } from '../../../components/RouterLink';
import { relatedRecordLinkProps } from '../linkedRecord';
import { smallChipIconSize } from '../../../components/iconSizes';

export const RelationshipSelectionPills = ({
  action,
  gap = 2,
  ids,
  labels,
  linkToRecords = false,
  onRemove,
  singleLine = false,
}: {
  action?: ReactNode;
  /** Spacing between pills, in theme units. */
  gap?: number;
  ids: string[];
  labels: ReadonlyMap<string, string>;
  /** Each pill opens its record in a new tab; removing one stays on the page. */
  linkToRecords?: boolean;
  onRemove?: (id: string) => void;
  /** Keeps the pills on one line, shortening long labels. */
  singleLine?: boolean;
}) => {
  if (ids.length === 0 && !action) return null;
  return (
    <Box
      sx={{
        alignItems: 'center',
        display: 'flex',
        flexWrap: singleLine ? 'nowrap' : 'wrap',
        gap,
        minWidth: 0,
      }}
    >
      {ids.map((id) =>
        linkToRecords ? (
          <RouterChip
            clickable
            icon={<NewTabIcon size={smallChipIconSize} />}
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
            sx={{ maxWidth: '100%', minWidth: singleLine ? 0 : undefined }}
            to="/records/$recordId"
            {...relatedRecordLinkProps}
          />
        ) : (
          <Chip
            key={id}
            label={labels.get(id) ?? id}
            onDelete={onRemove ? () => onRemove(id) : undefined}
            size="small"
            sx={{ maxWidth: '100%', minWidth: singleLine ? 0 : undefined }}
          />
        ),
      )}
      {action}
    </Box>
  );
};
