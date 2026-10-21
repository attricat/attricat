import { Box, Typography } from '@mui/material';
import { JsonBlock } from '../../components/JsonValue';

export const JsonMetadata = ({
  label,
  value,
}: {
  label: string;
  value: unknown;
}) => (
  <Box>
    <Typography
      color="text.secondary"
      sx={{ display: 'block', mb: 0.5 }}
      variant="caption"
    >
      {label}
    </Typography>
    <JsonBlock value={value} />
  </Box>
);
