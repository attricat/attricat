import { Alert } from '@mui/material';
import { errorMessage } from '../../api/errors';

export const SectionError = ({ error }: { error: Error | null }) =>
  error ? <Alert severity="error">{errorMessage(error)}</Alert> : null;
