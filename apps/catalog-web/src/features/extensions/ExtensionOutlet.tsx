import BoltOutlinedIcon from '@mui/icons-material/BoltOutlined';
import {
  Alert,
  Box,
  CircularProgress,
  IconButton,
  Popover,
  Stack,
  Tooltip,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { z } from 'zod';
import { ExtensionFrame } from './ExtensionFrame';
import { getExtensionRuntime, type ExtensionContribution } from './api';
import { extensionQueryKeys } from './query-keys';

type Outlet =
  | 'navigation'
  | 'entity_preview_panel'
  | 'blueprint_attribute_configuration'
  | 'entity_attribute_decoration'
  | 'entity_action'
  | 'explorer_row_action'
  | 'blueprint_detail_panel';

const contributionKey = (contribution: ExtensionContribution) =>
  `${contribution.extension_id}:${contribution.id}:${contribution.release_id}`;

const supportsOutlet = (contribution: ExtensionContribution, outlet: Outlet) =>
  contribution.outlet === outlet &&
  (contribution.kind === 'embedded' ||
    (outlet === 'explorer_row_action' && contribution.kind === 'action') ||
    (outlet === 'blueprint_detail_panel' && contribution.kind === 'panel'));

// New outlet contexts are deliberately small, strict, and versioned. They are
// the only page data an extension frame receives for these surfaces.
const outletContextSchemas = {
  explorer_row_action: z
    .object({
      blueprint_id: z.uuid(),
      blueprint_version: z.number().int().positive(),
      context_version: z.literal(1),
      entity_id: z.uuid(),
    })
    .strict(),
  blueprint_detail_panel: z
    .object({
      blueprint_id: z.uuid(),
      blueprint_version: z.number().int().positive(),
      context_version: z.literal(1),
    })
    .strict(),
};

const hasValidContext = (
  outlet: Outlet,
  context: Record<string, unknown> | undefined,
) => {
  const schema =
    outletContextSchemas[outlet as keyof typeof outletContextSchemas];
  return !schema || schema.safeParse(context).success;
};

type Props = {
  outlet: Outlet;
  context?: Record<string, unknown>;
};

/**
 * A fixed host-owned insertion point; extensions never choose a DOM selector.
 * Contributions from enabled extensions are independent; their relative order is
 * intentionally not a contract.
 */
export const ExtensionOutlet = ({ outlet, context }: Props) => {
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime,
    queryFn: getExtensionRuntime,
    // Runtime state can change outside this browser (safe mode, quarantine, or
    // grant revocation). Polling makes mounted frames unmount promptly; every
    // broker call remains server-gated between refreshes.
    refetchInterval: 15_000,
    retry: false,
  });
  if (runtime.isPending)
    return (
      <Box
        aria-live="polite"
        role="status"
        sx={{ display: 'flex', justifyContent: 'center', py: 1 }}
      >
        <CircularProgress aria-label="Loading extension content" size={20} />
      </Box>
    );
  if (runtime.isError)
    return (
      <Alert role="status" severity="warning">
        Extension content could not be loaded.
      </Alert>
    );
  return (
    <>
      <Stack spacing={1}>
        {runtime.data
          ?.filter(
            (item) =>
              supportsOutlet(item, outlet) && hasValidContext(outlet, context),
          )
          .map((item) => (
            <ExtensionFrame
              contribution={item}
              context={context}
              key={`${item.extension_id}:${item.id}:${item.release_id}`}
            />
          ))}
      </Stack>
    </>
  );
};

export const ExtensionPopoverOutlet = ({
  context,
  label,
  outlet,
}: {
  context?: Record<string, unknown>;
  label: string;
  outlet: Outlet;
}) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [contentHeights, setContentHeights] = useState<Record<string, number>>(
    {},
  );
  const contextKey = JSON.stringify(context);
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime,
    queryFn: getExtensionRuntime,
    refetchInterval: 15_000,
    retry: false,
  });
  const contributions =
    runtime.data?.filter(
      (item) =>
        supportsOutlet(item, outlet) && hasValidContext(outlet, context),
    ) ?? [];
  const contentKey = (contribution: ExtensionContribution) =>
    `${contextKey}:${contributionKey(contribution)}`;
  const hasContent = contributions.some(
    (contribution) => contentHeights[contentKey(contribution)] > 0,
  );
  if (!contributions.length) return null;
  return (
    <>
      {!hasContent && (
        <Box
          sx={{
            left: -10_000,
            position: 'fixed',
            top: 0,
            visibility: 'hidden',
            width: 480,
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
        </Box>
      )}
      {hasContent && (
        <Tooltip title={label}>
          <IconButton
            aria-label={label}
            onClick={(event) => setAnchor(event.currentTarget)}
            size="small"
          >
            <BoltOutlinedIcon fontSize="inherit" />
          </IconButton>
        </Tooltip>
      )}
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
      >
        <Stack sx={{ maxWidth: 480, p: 2 }}>
          <ExtensionOutlet context={context} outlet={outlet} />
        </Stack>
      </Popover>
    </>
  );
};

export const ExtensionRoutePage = ({
  extensionId,
  contributionId,
}: {
  extensionId: string;
  contributionId: string;
}) => {
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime,
    queryFn: getExtensionRuntime,
    refetchInterval: 15_000,
    retry: false,
  });
  if (runtime.isPending)
    return (
      <Box
        aria-live="polite"
        role="status"
        sx={{ display: 'flex', justifyContent: 'center', py: 2 }}
      >
        <CircularProgress aria-label="Loading extension page" />
      </Box>
    );
  if (runtime.isError)
    return (
      <Alert severity="warning">Extension content could not be loaded.</Alert>
    );
  const contribution = runtime.data?.find(
    (item) =>
      item.kind === 'route' &&
      item.extension_id === extensionId &&
      item.id === contributionId,
  );
  if (!contribution)
    return (
      <Alert severity="warning">This extension page is unavailable.</Alert>
    );
  return <ExtensionFrame contribution={contribution} />;
};
