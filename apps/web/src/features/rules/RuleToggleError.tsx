import { PowerIcon } from 'lucide-react';
import { Alert, Button, type SxProps, type Theme } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { canAcceptExistingViolations, dryRunGateMessage } from './dryRunGate';
import type { ToggleRule } from './useToggleRule';

/**
 * Why enabling or disabling a rule failed. When an enforcing rule's dry run
 * found existing violations or reached the record limit, offers to enable it
 * anyway.
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
  const violationsToAccept = canAcceptExistingViolations(toggle.error)
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
      {dryRunGateMessage(t, toggle.error)}
    </Alert>
  );
};
