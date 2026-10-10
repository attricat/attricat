import { Typography } from '@mui/material';

type Props = {
  /** Nests the heading under a panel's h2 record heading. */
  compact?: boolean;
  name?: string;
};

/** Names the schema used by the record page beneath its action toolbar. */
export const RecordSchemaSubheader = ({ compact = false, name }: Props) =>
  name ? (
    <Typography component={compact ? 'h3' : 'h2'} sx={{ mt: 3 }} variant="h6">
      {name}
    </Typography>
  ) : null;
