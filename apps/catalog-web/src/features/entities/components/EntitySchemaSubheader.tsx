import { Stack, Typography } from '@mui/material';
import { EntityIdPopover } from './EntityIdPopover';

type Props = {
  entityId: string;
  name?: string;
};

/** Identifies the schema used by the entity page beneath its action toolbar. */
export const EntitySchemaSubheader = ({ entityId, name }: Props) => (
  <Stack alignItems="baseline" direction="row" spacing={0.5} sx={{ mt: 3 }}>
    {name && (
      <Typography component="h2" variant="h6">
        {name}
      </Typography>
    )}
    <EntityIdPopover alignWithText entityId={entityId} />
  </Stack>
);
