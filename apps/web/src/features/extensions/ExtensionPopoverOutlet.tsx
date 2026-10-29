import {
  IconButton,
  ListItemIcon,
  ListItemText,
  MenuItem,
  Popover,
  Stack,
  Tooltip,
} from '@mui/material';
import { useState, type MouseEvent } from 'react';
import { ExtensionIcon } from '../../components/systemIcons';
import {
  ActionSelectionContext,
  type ActionSelection,
} from './actionSelection';
import type { ExtensionContribution, ExtensionRuntimeScope } from './api';
import {
  extensionPopoverTriggers,
  extensionPopoverWidth,
  type ExtensionPopoverTrigger,
} from './constants';
import { ExtensionFrame } from './ExtensionFrame';
import {
  contributionKey,
  outletContributions,
  type OutletContext,
  type OutletName,
} from './outletContributions';
import { useExtensionRuntime } from './useExtensionRuntime';
import { compactIconSize } from '../../components/iconSizes';

type ExtensionPopoverOutletProps = {
  context?: OutletContext;
  label: string;
  outlet: OutletName;
  runtimeScope?: ExtensionRuntimeScope;
  selection?: ActionSelection;
  /** A compact icon button by default; see `extensionPopoverTriggers`. */
  trigger?: ExtensionPopoverTrigger;
};

/** Mounts contributions off-screen and reveals a trigger once any renders content. */
export const ExtensionPopoverOutlet = ({
  context,
  label,
  outlet,
  runtimeScope,
  selection,
  trigger = extensionPopoverTriggers.icon,
}: ExtensionPopoverOutletProps) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [contentHeights, setContentHeights] = useState<Record<string, number>>(
    {},
  );
  const contextKey = JSON.stringify(context);
  const runtime = useExtensionRuntime(runtimeScope);
  const contributions = outletContributions(
    runtime.data,
    outlet,
    context,
    selection,
  );
  const contentKey = (contribution: ExtensionContribution) =>
    `${contextKey}:${contributionKey(contribution)}`;
  const hasContent = contributions.some(
    (contribution) => contentHeights[contentKey(contribution)] > 0,
  );
  if (!contributions.length) return null;
  const open = (event: MouseEvent<HTMLElement>) =>
    setAnchor(event.currentTarget);
  const toolbar = trigger === extensionPopoverTriggers.toolbar;
  const horizontal = toolbar ? 'right' : 'left';
  return (
    <ActionSelectionContext value={selection ?? null}>
      {hasContent && trigger === extensionPopoverTriggers.menuItem && (
        <MenuItem onClick={open}>
          <ListItemIcon>
            <ExtensionIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{label}</ListItemText>
        </MenuItem>
      )}
      {hasContent && trigger !== extensionPopoverTriggers.menuItem && (
        <Tooltip title={label}>
          <IconButton
            aria-label={label}
            onClick={open}
            size={toolbar ? 'medium' : 'small'}
          >
            {toolbar ? (
              <ExtensionIcon />
            ) : (
              <ExtensionIcon size={compactIconSize} />
            )}
          </IconButton>
        </Tooltip>
      )}
      {/* Keep the measuring frame in this popover so opening it never remounts
          the extension artifact. */}
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal, vertical: 'bottom' }}
        keepMounted
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
        transformOrigin={{ horizontal, vertical: 'top' }}
      >
        <Stack
          sx={{
            maxWidth: extensionPopoverWidth,
            p: 2,
            width: extensionPopoverWidth,
          }}
        >
          {contributions.map((contribution) => (
            <ExtensionFrame
              context={context}
              contribution={contribution}
              key={contributionKey(contribution)}
              onContentHeight={(height) =>
                setContentHeights((current) => ({
                  ...current,
                  [contentKey(contribution)]: height,
                }))
              }
            />
          ))}
        </Stack>
      </Popover>
    </ActionSelectionContext>
  );
};
