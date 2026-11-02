import { PowerIcon, PowerOffIcon, ShieldAlertIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { StatusChip, type StatusChipStyle } from '../../components/StatusChip';
import { installationStateLabelKey } from './extensionPageUtils';
import { installationStates, type InstallationState } from './constants';

const stateChip = (state: InstallationState): StatusChipStyle => {
  if (state === installationStates.enabled)
    return { color: 'success', icon: PowerIcon };
  if (state === installationStates.quarantined)
    return { color: 'error', icon: ShieldAlertIcon };
  return { color: 'default', icon: PowerOffIcon };
};

/** Whether an installed extension runs. */
export const InstallationStateChip = ({
  state,
}: {
  state: InstallationState;
}) => {
  const { t } = useTranslation();
  return (
    <StatusChip
      label={t(installationStateLabelKey(state))}
      size="small"
      style={stateChip(state)}
    />
  );
};
