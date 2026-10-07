import { Stack, Typography } from '@mui/material';
import { EntityIdPopover } from './EntityIdPopover';

type Props = {
  /** Nests the heading under a panel's h2 entity heading. */
  compact?: boolean;
  entityId: string;
  name?: string;
};

/** Identifies the schema used by the entity page beneath its action toolbar. */
export const EntitySchemaSubheader = ({
  compact = false,
  entityId,
  name,
}: Props) => (
  <Stack direction="row" spacing={0.5} sx={{ alignItems: 'baseline', mt: 3 }}>
    <EntityIdPopover alignWithText edge="start" entityId={entityId} />
    {name && (
      <Typography component={compact ? 'h3' : 'h2'} variant="h6">
        {name}
      </Typography>
    )}
  </Stack>
);
