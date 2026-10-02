import { Alert, Box, CircularProgress } from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  ActionSelectionContext,
  type ActionSelection,
} from './actionSelection';
import type { ExtensionRuntimeScope } from './api';
import { extensionLoadingIndicatorSize } from './constants';
import { ExtensionActionBarOutlet } from './ExtensionActionBarOutlet';
import { ExtensionAppsList } from './ExtensionAppsList';
import { ExtensionCardOutlet } from './ExtensionCardOutlet';
import { ExtensionNavigationOutlet } from './ExtensionNavigationOutlet';
import { ExtensionPanelOutlet } from './ExtensionPanelOutlet';
import {
  outletContributions,
  outletPolicies,
  type OutletContext,
  type OutletName,
} from './outletContributions';
import { useExtensionRuntime } from './useExtensionRuntime';

// Existing callers import every outlet surface from this module.
export { ExtensionPopoverOutlet } from './ExtensionPopoverOutlet';
export { ExtensionRoutePage } from './ExtensionRoutePage';

type Props = {
  outlet: OutletName;
  context?: OutletContext;
  navigationDisplay?: 'grouped' | 'all';
  canBrowseExtensions?: boolean;
  onBrowseExtensions?: () => void;
  onNavigate?: () => void;
  runtimeScope?: ExtensionRuntimeScope;
  /** Saved entities this action surface applies to (selection-aware outlets). */
  selection?: ActionSelection;
};

/**
 * A fixed host-owned insertion point; extensions never choose a DOM selector.
 * Contributions arrive in the host-computed display order; this component only
 * applies the fixed capacity, grouping, and overflow policy for the outlet.
 */
export const ExtensionOutlet = ({
  outlet,
  context,
  navigationDisplay = 'grouped',
  canBrowseExtensions = false,
  onBrowseExtensions,
  onNavigate,
  runtimeScope,
  selection,
}: Props) => {
  const { t } = useTranslation();
  const runtime = useExtensionRuntime(runtimeScope);
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
          size={extensionLoadingIndicatorSize}
        />
      </Box>
    );
  if (runtime.isError)
    return (
      <Alert role="status" severity="warning">
        {t('extensions.contentLoadFailed')}
      </Alert>
    );
  const contributions = outletContributions(
    runtime.data,
    outlet,
    context,
    selection,
  );
  const policy = outletPolicies[outlet];
  switch (policy.kind) {
    case 'card':
      return (
        <ExtensionCardOutlet
          context={context}
          contributions={contributions}
          policy={policy}
        />
      );
    case 'actionBar':
      return (
        <ActionSelectionContext value={selection ?? null}>
          <ExtensionActionBarOutlet
            context={context}
            contributions={contributions}
            policy={policy}
          />
        </ActionSelectionContext>
      );
    case 'navigation':
      return navigationDisplay === 'all' ? (
        <ExtensionAppsList
          canBrowseExtensions={canBrowseExtensions}
          context={context}
          contributions={contributions}
          onBrowseExtensions={onBrowseExtensions}
          onNavigate={onNavigate}
        />
      ) : (
        <ExtensionNavigationOutlet
          context={context}
          contributions={contributions}
          onNavigate={onNavigate}
          policy={policy}
        />
      );
    case 'panel':
      return (
        <ExtensionPanelOutlet
          context={context}
          contributions={contributions}
          policy={policy}
        />
      );
    default:
      return null;
  }
};
