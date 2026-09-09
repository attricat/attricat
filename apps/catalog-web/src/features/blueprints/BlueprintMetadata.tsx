import { Box, Typography } from '@mui/material';

export const JsonMetadata = ({
  label,
  value,
}: {
  label: string;
  value: unknown;
}) => (
  <Box>
    <Typography color="text.secondary" variant="caption">
      {label}
    </Typography>
    <Typography
      component="pre"
      sx={{
        fontFamily: 'monospace',
        fontSize: '0.75rem',
        m: 0,
        overflowX: 'auto',
        whiteSpace: 'pre-wrap',
      }}
    >
      {JSON.stringify(value, null, 2)}
    </Typography>
  </Box>
);
