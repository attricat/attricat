import { IconButton, Popover, Stack, Tooltip } from '@mui/material';
import { useState } from 'react';
import { ExtensionIcon } from '../../components/systemIcons';
import type { ExtensionContribution, ExtensionRuntimeScope } from './api';
import { extensionPopoverWidth } from './constants';
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
};

/** Mounts contributions off-screen and reveals a trigger once any renders content. */
export const ExtensionPopoverOutlet = ({
  context,
  label,
  outlet,
  runtimeScope,
}: ExtensionPopoverOutletProps) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [contentHeights, setContentHeights] = useState<Record<string, number>>(
    {},
  );
  const contextKey = JSON.stringify(context);
  const runtime = useExtensionRuntime(runtimeScope);
  const contributions = outletContributions(runtime.data, outlet, context);
  const contentKey = (contribution: ExtensionContribution) =>
    `${contextKey}:${contributionKey(contribution)}`;
  const hasContent = contributions.some(
    (contribution) => contentHeights[contentKey(contribution)] > 0,
  );
  if (!contributions.length) return null;
  return (
    <>
      {hasContent && (
        <Tooltip title={label}>
          <IconButton
            aria-label={label}
            onClick={(event) => setAnchor(event.currentTarget)}
            size="small"
          >
            <ExtensionIcon size={compactIconSize} />
          </IconButton>
        </Tooltip>
      )}
      {/* Keep the measuring frame in this popover so opening it never remounts
          the extension artifact. */}
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        keepMounted
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
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
    </>
  );
};
