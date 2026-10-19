import { Alert, AlertTitle, Box, Link } from '@mui/material';

export type FormErrorSummaryItem = {
  id: string;
  label: string;
  message: string;
};

/**
 * Lists the fields that stop a form from being saved, each a link to the
 * field, so errors in collapsed sections or far down the form are found.
 */
export const FormErrorSummary = ({
  title,
  errors,
  onSelect,
}: {
  title: string;
  errors: readonly FormErrorSummaryItem[];
  onSelect: (id: string) => void;
}) => (
  <Alert severity="error">
    <AlertTitle>{title}</AlertTitle>
    <Box component="ul" sx={{ m: 0, pl: 2 }}>
      {errors.map((error) => (
        <li key={error.id}>
          <Link
            color="inherit"
            component="button"
            onClick={() => onSelect(error.id)}
            sx={{ textAlign: 'start', verticalAlign: 'baseline' }}
            type="button"
            underline="always"
          >
            {error.label}
          </Link>
          {`: ${error.message}`}
        </li>
      ))}
    </Box>
  </Alert>
);
