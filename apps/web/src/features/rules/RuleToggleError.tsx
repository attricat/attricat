import { PowerIcon } from 'lucide-react';
import { Alert, Button, type SxProps, type Theme } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ApiRequestError } from '../../api/request';
import { errorMessage } from '../../api/errors';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { RULE_HAS_EXISTING_VIOLATIONS } from './constants';
import type { ToggleRule } from './useToggleRule';

/**
 * Why enabling or disabling a rule failed. When an enforcing rule's dry run
 * found existing violations, offers to enable it anyway.
 */
export const RuleToggleError = ({
  sx,
  toggle,
}: {
  sx?: SxProps<Theme>;
  toggle: ToggleRule;
}) => {
  const { t } = useTranslation();
  if (!toggle.error) return null;
  const violationsToAccept =
    toggle.error instanceof ApiRequestError &&
    toggle.error.code === RULE_HAS_EXISTING_VIOLATIONS
      ? toggle.variables
      : undefined;
  if (!violationsToAccept)
    return <ApiErrorAlert error={toggle.error} sx={sx} />;
  return (
    <Alert
      action={
        <Button
          color="inherit"
          disabled={toggle.isPending}
          onClick={() =>
            toggle.mutate({
              ...violationsToAccept,
              acceptExistingViolations: true,
            })
          }
          size="small"
          startIcon={<PowerIcon />}
        >
          {t('rules.enableAnyway')}
        </Button>
      }
      severity="warning"
      sx={sx}
    >
      {errorMessage(toggle.error)}
    </Alert>
  );
};
