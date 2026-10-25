import { Alert } from '@mui/material';
import { errorMessage } from '../../api/errors';

export const ErrorNotice = ({ error }: { error: Error | null }) =>
  error ? (
    <Alert severity="error" sx={{ mb: 2 }}>
      {errorMessage(error)}
    </Alert>
  ) : null;
