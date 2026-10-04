import { Alert, AlertTitle, Box, Typography } from '@mui/material';
import type { SxProps, Theme } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { CheckViolation } from '../features/entities/schemas';
import { checkViolationError } from '../features/entities/checkViolations';
import { useViolationText } from './useViolationText';

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

/**
 * Shows an API failure, listing its check violations when it has any. Pass
 * `violations` to list a subset, for example those not shown on form fields.
 */
export const ApiErrorAlert = ({
  error,
  violations,
  sx,
}: {
  error: Error;
  violations?: readonly CheckViolation[];
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
      violations={violations ?? checks.violations}
    />
  );
};
