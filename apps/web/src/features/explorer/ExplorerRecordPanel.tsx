import {
  Box,
  ClickAwayListener,
  IconButton,
  Paper,
  Tooltip,
} from '@mui/material';
import { Maximize2Icon, XIcon } from 'lucide-react';
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { compactIconSize } from '../../components/iconSizes';
import { RecordPreview } from '../records/components/RecordPreview';
import { RecordCommentsLink } from '../record-comments/RecordCommentsLink';
import { lexiconText } from '../lexicon/lexicon';
import { recordPanelOpenerAttribute, recordPanelWidth } from './constants';

/** Another result's link switches the panel instead of closing it. */
const closesPanelOnClick = (event: MouseEvent | TouchEvent) =>
  !(
    event.target instanceof Element &&
    event.target.closest(`[${recordPanelOpenerAttribute}]`)
  );

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
  recordId: string;
  onClose: () => void;
  /** Shows another record in the panel, such as a new duplicate. */
  onOpenRecord: (recordId: string) => void;
};

/**
 * The record page's preview and actions in a panel floating over the
 * Explorer results.
 */
export const ExplorerRecordPanel = ({
  contextId,
  recordId,
  onClose,
  onOpenRecord,
}: Props) => {
  const { t } = useTranslation();
  const panelRef = useRef<HTMLElement>(null);

  // Moves keyboard and screen reader users to the record they opened.
  useEffect(() => {
    panelRef.current?.focus();
  }, [recordId]);
  // A press that starts in the panel, such as one opening a select whose
  // backdrop then takes the release, ends in a click on the page body.
  // Document listeners run before React's, which mark presses inside.
  const pressedInside = useRef(false);
  useEffect(() => {
    const resetPress = () => {
      pressedInside.current = false;
    };
    document.addEventListener('pointerdown', resetPress, true);
    return () => document.removeEventListener('pointerdown', resetPress, true);
  }, []);

  return (
    // Dialogs, drawers and menus opened from the panel are portalled but
    // still count as inside it.
    <ClickAwayListener
      onClickAway={(event) => {
        // A press answers one click only; a later keyboard click outside, which
        // has no press, still closes the panel.
        const startedInside = pressedInside.current;
        pressedInside.current = false;
        if (!startedInside && closesPanelOnClick(event)) onClose();
      }}
    >
      {/* Holds the panel without affecting its fixed layout. */}
      <Box
        onPointerDownCapture={() => {
          pressedInside.current = true;
        }}
        sx={{ display: 'contents' }}
      >
        <RecordPreview
          compact
          drawerOffset={recordPanelWidth}
          recordId={recordId}
          initialContextId={contextId}
          // Context, dialogs and unsaved edits belong to one record.
          key={recordId}
          onDeleted={onClose}
          onDuplicated={(copy) => onOpenRecord(copy.id)}
          renderFrame={({ blueprint, children, headerActions }) => (
            <Paper
              // Named for assistive technology only; the record heading inside
              // already identifies it visually.
              aria-label={
                blueprint
                  ? t('explorer.recordPanelTitle', {
                      blueprint: lexiconText(blueprint.name),
                    })
                  : t('records.recordPreview')
              }
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
                width: recordPanelWidth,
                zIndex: 'drawer',
              }}
              tabIndex={-1}
            >
              <Box
                sx={{
                  alignItems: 'center',
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: 1,
                  mb: 2,
                }}
              >
                {headerActions}
                <RecordCommentsLink recordId={recordId} />
                <RouterButton
                  params={{ recordId }}
                  size="small"
                  startIcon={<Maximize2Icon size={compactIconSize} />}
                  sx={{ ml: 'auto' }}
                  to="/records/$recordId"
                >
                  {t('explorer.openFullPage')}
                </RouterButton>
                <Tooltip title={t('explorer.closeRecordPanel')}>
                  <IconButton
                    aria-label={t('explorer.closeRecordPanel')}
                    onClick={onClose}
                    size="small"
                  >
                    <XIcon size={compactIconSize} />
                  </IconButton>
                </Tooltip>
              </Box>
              {children}
            </Paper>
          )}
        />
      </Box>
    </ClickAwayListener>
  );
};
