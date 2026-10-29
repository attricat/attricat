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
import {
  ActionMenuCloseContext,
  useCloseActionMenu,
  type ActionTarget,
} from './actions';
import type { ExtensionContribution, ExtensionRuntimeScope } from './api';
import {
  contributionKinds,
  outletCapacities,
  type ActionOutlet,
} from './constants';
import { contributionKey } from './outletContributions';
import { useExtensionRuntime } from './useExtensionRuntime';
import { useInvokeExtensionAction } from './useInvokeExtensionAction';

type ActionsProps = {
  outlet: ActionOutlet;
  runtimeScope: ExtensionRuntimeScope;
  target: ActionTarget;
};

/** The enabled actions placed in one outlet, in host-computed order. */
const useOutletActions = (
  outlet: ActionOutlet,
  runtimeScope: ExtensionRuntimeScope,
) =>
  useExtensionRuntime(runtimeScope).data?.filter(
    (item) =>
      item.kind === contributionKinds.action &&
      item.outlet === outlet &&
      item.invoke !== null,
  ) ?? [];

const actionTitle = (action: ExtensionContribution) =>
  action.title ?? action.id;

/**
 * An outlet's actions as items of the menu they are placed in. Choosing one
 * closes that menu, then opens the action's dialog or calls its command.
 */
export const ExtensionActionMenuItems = ({
  outlet,
  runtimeScope,
  target,
}: ActionsProps) => {
  const actions = useOutletActions(outlet, runtimeScope);
  const invoke = useInvokeExtensionAction();
  const closeMenu = useCloseActionMenu();
  return actions.map((action) => (
    <MenuItem
      key={contributionKey(action)}
      onClick={() => {
        closeMenu?.();
        void invoke(action, outlet, target);
      }}
    >
      <ListItemIcon>
        <ExtensionIcon size={compactIconSize} />
      </ListItemIcon>
      <ListItemText>{actionTitle(action)}</ListItemText>
    </MenuItem>
  ));
};

/** A toolbar button that lists an outlet's actions, shown only when it has any. */
export const ExtensionActionsMenuButton = ({
  label,
  ...props
}: ActionsProps & { label: string }) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const actions = useOutletActions(props.outlet, props.runtimeScope);
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
 * decides how many show; the rest wait behind a "more" menu.
 */
export const ExtensionActionButtons = (props: ActionsProps) => {
  const { t } = useTranslation();
  const actions = useOutletActions(props.outlet, props.runtimeScope);
  const invoke = useInvokeExtensionAction();
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
      {visible.map((action) => (
        <Button
          disabled={running === contributionKey(action)}
          key={contributionKey(action)}
          onClick={() => void run(action)}
          size="small"
          startIcon={<ExtensionIcon size={compactIconSize} />}
          variant="outlined"
        >
          {actionTitle(action)}
        </Button>
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
            {overflow.map((action) => (
              <MenuItem
                key={contributionKey(action)}
                onClick={() => {
                  setOverflowAnchor(null);
                  void run(action);
                }}
              >
                <ListItemText>{actionTitle(action)}</ListItemText>
              </MenuItem>
            ))}
          </Menu>
        </>
      )}
    </Stack>
  );
};
