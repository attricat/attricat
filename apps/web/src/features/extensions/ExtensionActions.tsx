import {
  Button,
  IconButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Stack,
  Tooltip,
} from '@mui/material';
import { EllipsisIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { ExtensionIcon } from '../../components/systemIcons';
import { ActionMenuCloseContext, useCloseActionMenu } from './actions';
import type { ExtensionContribution } from './api';
import { outletCapacities } from './constants';
import { useContributionTitle } from './contributionTitle';
import { renderHostIcon } from './hostIcons';
import { contributionKey } from './outletContributions';
import { useInvokeExtensionAction } from './useInvokeExtensionAction';
import { useOfferedActions, type ActionsProps } from './useOfferedActions';

const ActionIcon = ({ action }: { action: ExtensionContribution }) =>
  renderHostIcon(action.icon, compactIconSize);

/**
 * An outlet's actions as items of the menu they are placed in. Choosing one
 * closes that menu, then runs the action.
 */
export const ExtensionActionMenuItems = (props: ActionsProps) => {
  const actions = useOfferedActions(props);
  const invoke = useInvokeExtensionAction();
  const closeMenu = useCloseActionMenu();
  const titleOf = useContributionTitle();
  return actions.map(({ action, availability }) => (
    <MenuItem
      disabled={availability.disabledReason !== undefined}
      key={contributionKey(action)}
      onClick={() => {
        closeMenu?.();
        void invoke(action, props.outlet, props.target);
      }}
    >
      <ListItemIcon>
        <ActionIcon action={action} />
      </ListItemIcon>
      <ListItemText secondary={availability.disabledReason}>
        {titleOf(action)}
      </ListItemText>
    </MenuItem>
  ));
};

/** A toolbar button that lists an outlet's actions, shown only when it has any. */
export const ExtensionActionsMenuButton = ({
  label,
  ...props
}: ActionsProps & { label: string }) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const actions = useOfferedActions(props);
  if (!actions.length) return null;
  return (
    <>
      <Tooltip title={label}>
        <IconButton
          aria-label={label}
          onClick={(event) => setAnchor(event.currentTarget)}
          size="medium"
        >
          <ExtensionIcon />
        </IconButton>
      </Tooltip>
      <Menu
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
        transformOrigin={{ horizontal: 'right', vertical: 'top' }}
      >
        <ActionMenuCloseContext value={() => setAnchor(null)}>
          <ExtensionActionMenuItems {...props} />
        </ActionMenuCloseContext>
      </Menu>
    </>
  );
};

/**
 * An outlet's actions as a row of buttons. The host's action-bar capacity
 * decides how many show; the rest wait behind a "more" menu. A disabled
 * action says why in its tooltip.
 */
export const ExtensionActionButtons = (props: ActionsProps) => {
  const { t } = useTranslation();
  const actions = useOfferedActions(props);
  const invoke = useInvokeExtensionAction();
  const titleOf = useContributionTitle();
  const [running, setRunning] = useState<string>();
  const [overflowAnchor, setOverflowAnchor] = useState<HTMLElement | null>(
    null,
  );
  const capacity =
    outletCapacities.actionBarPrimary + outletCapacities.actionBarSecondary;
  const visible = actions.slice(0, capacity);
  const overflow = actions.slice(capacity);
  if (!actions.length) return null;
  const run = async (action: ExtensionContribution) => {
    setRunning(contributionKey(action));
    try {
      await invoke(action, props.outlet, props.target);
    } finally {
      setRunning(undefined);
    }
  };
  return (
    <Stack
      direction="row"
      sx={{ alignItems: 'center', flexWrap: 'wrap', gap: 1 }}
    >
      {visible.map(({ action, availability }) => (
        <Tooltip
          key={contributionKey(action)}
          title={availability.disabledReason ?? ''}
        >
          {/* A disabled button fires no events; the span keeps the tooltip. */}
          <span>
            <Button
              disabled={
                running === contributionKey(action) ||
                availability.disabledReason !== undefined
              }
              onClick={() => void run(action)}
              size="small"
              startIcon={<ActionIcon action={action} />}
              variant="outlined"
            >
              {titleOf(action)}
            </Button>
          </span>
        </Tooltip>
      ))}
      {overflow.length > 0 && (
        <>
          <Tooltip title={t('extensions.moreActions')}>
            <IconButton
              aria-label={t('extensions.moreActions')}
              onClick={(event) => setOverflowAnchor(event.currentTarget)}
              size="small"
            >
              <EllipsisIcon size={compactIconSize} />
            </IconButton>
          </Tooltip>
          <Menu
            anchorEl={overflowAnchor}
            onClose={() => setOverflowAnchor(null)}
            open={Boolean(overflowAnchor)}
          >
            {overflow.map(({ action, availability }) => (
              <MenuItem
                disabled={availability.disabledReason !== undefined}
                key={contributionKey(action)}
                onClick={() => {
                  setOverflowAnchor(null);
                  void run(action);
                }}
              >
                <ListItemIcon>
                  <ActionIcon action={action} />
                </ListItemIcon>
                <ListItemText secondary={availability.disabledReason}>
                  {titleOf(action)}
                </ListItemText>
              </MenuItem>
            ))}
          </Menu>
        </>
      )}
    </Stack>
  );
};
