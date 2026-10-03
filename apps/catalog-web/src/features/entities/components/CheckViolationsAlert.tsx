import { Alert, AlertTitle, Box, Typography } from '@mui/material';
import type { SxProps, Theme } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { CheckViolation } from '../schemas';
import { checkViolationError } from '../checkViolations';

/** One violation's message, naming the contexts in which it failed. */
export const useViolationText = () => {
  const { t } = useTranslation();
  return (violation: CheckViolation) =>
    violation.contexts.length > 0
      ? t('entities.checks.violationInContexts', {
          message: violation.message,
          contexts: violation.contexts.join(', '),
        })
      : violation.message;
};

type Props = {
  title: string;
  violations: readonly CheckViolation[];
  sx?: SxProps<Theme>;
};

/** Lists failed checks, conditions or rules reported by the API. */
export const CheckViolationsAlert = ({ title, violations, sx }: Props) => {
  const violationText = useViolationText();
  return (
    <Alert severity="error" sx={sx}>
      <AlertTitle>{title}</AlertTitle>
      {violations.length > 0 && (
        <Box component="ul" sx={{ my: 0, pl: 3 }}>
          {violations.map((violation, index) => (
            <li key={`${violation.source}:${violation.code}:${index}`}>
              <Typography variant="body2">
                {violationText(violation)}
              </Typography>
            </li>
          ))}
        </Box>
      )}
    </Alert>
  );
};

/** Shows an API failure, listing its check violations when it has any. */
export const ApiErrorAlert = ({
  error,
  sx,
}: {
  error: Error;
  sx?: SxProps<Theme>;
}) => {
  const { t } = useTranslation();
  const checks = checkViolationError(error);
  if (!checks)
    return (
      <Alert severity="error" sx={sx}>
        {error.message}
      </Alert>
    );
  return (
    <CheckViolationsAlert
      sx={sx}
      title={
        checks.context
          ? t('entities.checks.publicationBlocked', { context: checks.context })
          : checks.message
      }
      violations={checks.violations}
    />
  );
};
