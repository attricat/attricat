import {
  Dialog,
  DialogContent,
  DialogTitle,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  TextField,
  Typography,
} from '@mui/material';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import {
  useActionPaletteStore,
  type PaletteSurface,
} from './actionPaletteStore';
import { useContributionTitle } from './contributionTitle';
import { renderHostIcon } from './hostIcons';
import { contributionKey } from './outletContributions';
import { useInvokeExtensionAction } from './useInvokeExtensionAction';
import { useOfferedActions } from './useOfferedActions';

/** The key that opens the palette, with Ctrl or ⌘. */
const paletteKey = 'k';

const SurfaceActions = ({
  query,
  surface,
  onChosen,
}: {
  query: string;
  surface: PaletteSurface;
  onChosen: () => void;
}) => {
  const actions = useOfferedActions(surface);
  const invoke = useInvokeExtensionAction();
  const titleOf = useContributionTitle();
  const needle = query.trim().toLocaleLowerCase();
  return actions
    .filter(({ action }) =>
      titleOf(action).toLocaleLowerCase().includes(needle),
    )
    .map(({ action, availability }) => (
      <ListItemButton
        disabled={availability.disabledReason !== undefined}
        key={`${surface.outlet}:${contributionKey(action)}`}
        onClick={() => {
          onChosen();
          void invoke(action, surface.outlet, surface.target);
        }}
      >
        <ListItemIcon>
          {renderHostIcon(action.icon, compactIconSize)}
        </ListItemIcon>
        <ListItemText
          primary={titleOf(action)}
          secondary={availability.disabledReason ?? action.extension_name}
        />
      </ListItemButton>
    ));
};

/**
 * Ctrl/⌘+K lists the extension actions the current page offers, the same
 * ones its menus and buttons show, and runs the chosen one.
 */
export const ActionPalette = () => {
  const { t } = useTranslation();
  const open = useActionPaletteStore((state) => state.open);
  const setOpen = useActionPaletteStore((state) => state.setOpen);
  const surfaces = useActionPaletteStore((state) => state.surfaces);
  const [query, setQuery] = useState('');
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (
        event.key.toLowerCase() === paletteKey &&
        (event.ctrlKey || event.metaKey) &&
        !event.altKey
      ) {
        event.preventDefault();
        setQuery('');
        setOpen(true);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [setOpen]);
  // Focuses the search field once, when the dialog opens.
  const focusOnOpen = useCallback(
    (input: HTMLInputElement | null) => input?.focus(),
    [],
  );
  const close = () => setOpen(false);
  const entries = Object.entries(surfaces);
  return (
    <Dialog fullWidth maxWidth="sm" onClose={close} open={open}>
      <DialogTitle>{t('extensions.paletteTitle')}</DialogTitle>
      <DialogContent>
        <TextField
          fullWidth
          inputRef={focusOnOpen}
          label={t('extensions.paletteSearch')}
          onChange={(event) => setQuery(event.target.value)}
          size="small"
          sx={{ mt: 1 }}
          value={query}
        />
        <List aria-label={t('extensions.paletteTitle')} dense>
          {entries.map(([id, surface]) => (
            <SurfaceActions
              key={id}
              onChosen={close}
              query={query}
              surface={surface}
            />
          ))}
        </List>
        {!entries.length && (
          <Typography color="text.secondary" variant="body2">
            {t('extensions.paletteEmpty')}
          </Typography>
        )}
      </DialogContent>
    </Dialog>
  );
};
