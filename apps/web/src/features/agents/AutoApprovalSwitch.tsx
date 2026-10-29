import {
  Box,
  FormControlLabel,
  IconButton,
  Switch,
  Tooltip,
} from '@mui/material';
import { InfoIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';

/**
 * Lets the agent apply its edits and comments on a conversation's record
 * without asking, for the messages of the person who switches it on.
 */
export const AutoApprovalSwitch = ({
  checked,
  disabled = false,
  heldByOther = false,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  /** Someone else switched it on for their own messages. */
  heldByOther?: boolean;
  onChange: (enabled: boolean) => void;
}) => {
  const { t } = useTranslation();
  const about = heldByOther
    ? `${t('agents.autoApproval.heldByOther')} ${t('agents.autoApproval.hint')}`
    : t('agents.autoApproval.hint');
  return (
    <Box sx={{ alignItems: 'center', display: 'flex', mb: 1.5, px: 1 }}>
      <FormControlLabel
        control={
          <Switch
            checked={checked}
            disabled={disabled}
            onChange={(event) => onChange(event.target.checked)}
          />
        }
        label={t('agents.autoApproval.label')}
        slotProps={{ typography: { variant: 'body2' } }}
        sx={{ gap: 2, ml: 0, mr: 0.5 }}
      />
      <Tooltip title={about}>
        <IconButton aria-label={t('agents.autoApproval.about')}>
          <InfoIcon aria-hidden size={compactIconSize} />
        </IconButton>
      </Tooltip>
    </Box>
  );
};
