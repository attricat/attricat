import MoreHorizIcon from '@mui/icons-material/MoreHoriz';
import { ExtensionIcon } from '../../components/system-icons';
import {
  Alert,
  Box,
  CircularProgress,
  Divider,
  IconButton,
  Popover,
  Stack,
  Tooltip,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { z } from 'zod';
import { ExtensionFrame } from './ExtensionFrame';
import {
  getExtensionRuntime,
  type ExtensionContribution,
  type ExtensionRuntimeScope,
} from './api';
import { extensionQueryKeys } from './query-keys';
import { extensionRuntimeRefetchInterval } from './constants';

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

// Every mounted surface has an explicit host policy. Extension manifests never
// select capacity, grouping, or overflow behavior.
const outletPolicies = {
  navigation: { kind: 'navigation', promotedCapacity: 3 },
  entity_preview_panel: { kind: 'panel', visibleCapacity: 3 },
  blueprint_attribute_configuration: { kind: 'panel', visibleCapacity: 3 },
  entity_attribute_decoration: { kind: 'popover' },
  entity_action: {
    kind: 'actionBar',
    primaryCapacity: 1,
    secondaryCapacity: 3,
  },
  explorer_row_action: { kind: 'popover' },
  blueprint_detail_panel: { kind: 'panel', visibleCapacity: 3 },
} as const;

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
  runtimeScope?: ExtensionRuntimeScope;
};

/**
 * A fixed host-owned insertion point; extensions never choose a DOM selector.
 * Contributions arrive in the host-computed display order; this component only
 * applies the fixed capacity, grouping, and overflow policy for the outlet.
 */
export const ExtensionOutlet = ({ outlet, context, runtimeScope }: Props) => {
  const { t } = useTranslation();
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime(runtimeScope),
    queryFn: () => getExtensionRuntime(runtimeScope),
    // Runtime state can change outside this browser (safe mode, quarantine, or
    // grant revocation). Polling makes mounted frames unmount promptly; every
    // broker call remains server-gated between refreshes.
    refetchInterval: extensionRuntimeRefetchInterval,
    retry: false,
  });
  if (runtime.isPending)
    return (
      <Box
        aria-live="polite"
        role="status"
        sx={{ display: 'flex', justifyContent: 'center', py: 1 }}
      >
        <CircularProgress
          aria-label={t('extensions.loadingContent')}
          enableTrackSlot
          size={20}
        />
      </Box>
    );
  if (runtime.isError)
    return (
      <Alert role="status" severity="warning">
        {t('extensions.contentLoadFailed')}
      </Alert>
    );
  const contributions =
    runtime.data?.filter(
      (item) =>
        supportsOutlet(item, outlet) && hasValidContext(outlet, context),
    ) ?? [];
  const policy = outletPolicies[outlet];
  if (policy.kind === 'actionBar') {
    const visibleCapacity = policy.primaryCapacity + policy.secondaryCapacity;
    const primary = contributions.slice(0, policy.primaryCapacity);
    const secondary = contributions.slice(
      policy.primaryCapacity,
      visibleCapacity,
    );
    const overflow = contributions.slice(visibleCapacity);
    return (
      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
        {primary.map((item) => (
          <ExtensionFrame
            contribution={item}
            context={context}
            key={contributionKey(item)}
          />
        ))}
        {secondary.length > 0 && primary.length > 0 && (
          <Divider flexItem orientation="vertical" />
        )}
        {secondary.map((item) => (
          <ExtensionFrame
            contribution={item}
            context={context}
            key={contributionKey(item)}
          />
        ))}
        {overflow.length > 0 && (
          <ExtensionActionOverflow
            context={context}
            contributions={overflow}
            label={t('extensions.moreActions')}
          />
        )}
      </Stack>
    );
  }
  if (policy.kind === 'navigation') {
    const promoted = contributions.filter(
      (item) => item.navigation_group === 'promoted',
    );
    const visible = promoted.slice(0, policy.promotedCapacity);
    const visibleKeys = new Set(visible.map((item) => item.contribution_key));
    const grouped = contributions.filter(
      (item) => !visibleKeys.has(item.contribution_key),
    );
    return (
      <Stack spacing={1}>
        {visible.map((item) => (
          <ExtensionFrame
            contribution={item}
            context={context}
            key={contributionKey(item)}
          />
        ))}
        {grouped.length > 0 && (
          <ExtensionActionOverflow
            context={context}
            contributions={grouped}
            label={t('extensions.groupedNavigation')}
          />
        )}
      </Stack>
    );
  }
  if (policy.kind !== 'panel') return null;
  const visible = contributions.slice(0, policy.visibleCapacity);
  const overflow = contributions.slice(policy.visibleCapacity);
  return (
    <Stack spacing={1}>
      {visible.map((item) => (
        <ExtensionFrame
          contribution={item}
          context={context}
          key={contributionKey(item)}
        />
      ))}
      {overflow.length > 0 && (
        <ExtensionActionOverflow
          context={context}
          contributions={overflow}
          label={t('extensions.moreContent')}
        />
      )}
    </Stack>
  );
};

const ExtensionActionOverflow = ({
  context,
  contributions,
  label,
}: {
  context?: Record<string, unknown>;
  contributions: ExtensionContribution[];
  label: string;
}) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <Tooltip title={label}>
        <IconButton
          aria-label={label}
          onClick={(event) => setAnchor(event.currentTarget)}
          size="small"
        >
          <MoreHorizIcon fontSize="inherit" />
        </IconButton>
      </Tooltip>
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
      >
        <Stack spacing={1} sx={{ p: 1 }}>
          {contributions.map((contribution) => (
            <ExtensionFrame
              context={context}
              contribution={contribution}
              key={contributionKey(contribution)}
            />
          ))}
        </Stack>
      </Popover>
    </>
  );
};

export const ExtensionPopoverOutlet = ({
  context,
  label,
  outlet,
  runtimeScope,
}: {
  context?: Record<string, unknown>;
  label: string;
  outlet: Outlet;
  runtimeScope?: ExtensionRuntimeScope;
}) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [contentHeights, setContentHeights] = useState<Record<string, number>>(
    {},
  );
  const contextKey = JSON.stringify(context);
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime(runtimeScope),
    queryFn: () => getExtensionRuntime(runtimeScope),
    refetchInterval: extensionRuntimeRefetchInterval,
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
      {hasContent && (
        <Tooltip title={label}>
          <IconButton
            aria-label={label}
            onClick={(event) => setAnchor(event.currentTarget)}
            size="small"
          >
            <ExtensionIcon fontSize="inherit" />
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
        <Stack sx={{ maxWidth: 480, p: 2, width: 480 }}>
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

export const ExtensionRoutePage = ({
  extensionId,
  contributionId,
}: {
  extensionId: string;
  contributionId: string;
}) => {
  const { t } = useTranslation();
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime(),
    queryFn: () => getExtensionRuntime(),
    refetchInterval: extensionRuntimeRefetchInterval,
    retry: false,
  });
  if (runtime.isPending)
    return (
      <Box
        aria-live="polite"
        role="status"
        sx={{ display: 'flex', justifyContent: 'center', py: 2 }}
      >
        <CircularProgress
          aria-label={t('extensions.loadingPage')}
          enableTrackSlot
        />
      </Box>
    );
  if (runtime.isError)
    return (
      <Alert severity="warning">{t('extensions.contentLoadFailed')}</Alert>
    );
  const contribution = runtime.data?.find(
    (item) =>
      item.kind === 'route' &&
      item.extension_id === extensionId &&
      item.id === contributionId,
  );
  if (!contribution)
    return <Alert severity="warning">{t('extensions.pageUnavailable')}</Alert>;
  return <ExtensionFrame contribution={contribution} />;
};
