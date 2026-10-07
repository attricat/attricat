import {
  Alert,
  Box,
  CircularProgress,
  IconButton,
  Paper,
  Tooltip,
  Typography,
} from '@mui/material';
import { Maximize2Icon, XIcon } from 'lucide-react';
import { useEffect, useId, useRef, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RouterButton } from '../../components/RouterLink';
import { compactIconSize } from '../../components/iconSizes';
import { EntityHeading } from '../entities/components/EntityHeading';
import { EntityPreviewContent } from '../entities/components/EntityPreviewContent';
import { statusParentContexts } from '../entities/status';
import { useEntityContextSelection } from '../entities/useEntityContexts';
import { useEntityPreviewData } from '../entities/useEntityPreviewData';
import { lexiconText } from '../lexicon/lexicon';
import { entityPanelWidth } from './constants';

/** Escape inside a field reverts it; menus opened from the panel are portalled. */
const closesPanelOnEscape = (event: KeyboardEvent<HTMLElement>) =>
  event.key === 'Escape' &&
  !event.defaultPrevented &&
  event.target instanceof Element &&
  event.currentTarget.contains(event.target) &&
  !event.target.closest(
    'input, textarea, [contenteditable="true"], [role="combobox"]',
  );

type Props = {
  /** The Explorer's context, shown until another is chosen in the panel. */
  contextId?: string;
  entityId: string;
  onClose: () => void;
};

/** Shows and edits an Explorer result in a panel floating over the results. */
export const ExplorerEntityPanel = ({
  contextId: explorerContextId,
  entityId,
  onClose,
}: Props) => {
  const { t } = useTranslation();
  const titleId = useId();
  const panelRef = useRef<HTMLElement>(null);
  const { contextId, contexts, defaultContextId, setSelectedContext } =
    useEntityContextSelection(explorerContextId);
  const { blueprint, entityForm, resolved, statusTransitions } =
    useEntityPreviewData(entityId, contextId);

  // Moves keyboard and screen reader users to the entity they opened.
  useEffect(() => {
    panelRef.current?.focus();
  }, [entityId]);

  return (
    <Paper
      aria-labelledby={titleId}
      component="aside"
      // Overlay elevation; the panel floats over the results without a
      // backdrop so other rows stay clickable.
      elevation={16}
      onKeyDown={(event) => {
        if (closesPanelOnEscape(event)) onClose();
      }}
      ref={panelRef}
      sx={{
        borderLeft: 1,
        borderLeftColor: 'divider',
        borderRadius: 0,
        height: '100dvh',
        outline: 'none',
        overflowY: 'auto',
        position: 'fixed',
        px: 4,
        py: 6,
        right: 0,
        top: 0,
        width: entityPanelWidth,
        zIndex: 'drawer',
      }}
      tabIndex={-1}
    >
      <Box sx={{ alignItems: 'center', display: 'flex', gap: 1, mb: 2 }}>
        <Typography
          color="text.secondary"
          id={titleId}
          noWrap
          sx={{ flexGrow: 1 }}
          variant="body2"
        >
          {blueprint.data
            ? t('explorer.entityPanelTitle', {
                blueprint: lexiconText(blueprint.data.blueprint.name),
              })
            : t('entities.entityPreview')}
        </Typography>
        <RouterButton
          params={{ entityId }}
          size="small"
          startIcon={<Maximize2Icon size={compactIconSize} />}
          to="/entities/$entityId"
        >
          {t('explorer.openFullPage')}
        </RouterButton>
        <Tooltip title={t('explorer.closeEntityPanel')}>
          <IconButton
            aria-label={t('explorer.closeEntityPanel')}
            onClick={onClose}
            size="small"
          >
            <XIcon size={compactIconSize} />
          </IconButton>
        </Tooltip>
      </Box>
      {resolved.data && blueprint.data && (
        <EntityHeading
          attributes={blueprint.data.attributes}
          compact
          entityId={entityId}
          values={resolved.data.values}
          view={blueprint.data.blueprint.views.detail}
        />
      )}
      <QueryErrorNotice
        error={contexts.error}
        isRetrying={contexts.isFetching}
        onRetry={() => void contexts.refetch()}
      />
      {(contexts.isPending ||
        resolved.isPending ||
        (resolved.data && blueprint.isPending)) &&
        !contexts.isError && (
          <Box sx={{ display: 'flex', justifyContent: 'center', py: 3 }}>
            <CircularProgress
              aria-label={t('entities.resolvingValues')}
              enableTrackSlot
            />
          </Box>
        )}
      {resolved.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {resolved.error.message}
        </Alert>
      )}
      {blueprint.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {blueprint.error.message}
        </Alert>
      )}
      {contexts.data && resolved.data && blueprint.data && (
        <EntityPreviewContent
          blueprint={blueprint.data}
          contextId={contextId ?? undefined}
          contexts={contexts.data}
          contextsPending={contexts.isPending}
          defaultContextId={defaultContextId}
          entityId={entityId}
          // A cached form must not become the save baseline before the
          // opening refresh has completed.
          form={entityForm.isFetchedAfterMount ? entityForm.data : undefined}
          statusParentContextIds={statusParentContexts(
            contexts.data,
            contextId,
          )}
          statusTransitions={statusTransitions.data}
          onContextChange={setSelectedContext}
          resolved={resolved.data}
        />
      )}
    </Paper>
  );
};
