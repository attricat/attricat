import CloseIcon from '@mui/icons-material/Close';
import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  Stack,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import type { ReactNode } from 'react';

type Props = {
  applyDisabled?: boolean;
  applyLabel: string;
  cancelLabel: string;
  children: ReactNode;
  clearLabel: string;
  closeLabel: string;
  onApply: () => void;
  onClear: () => void;
  hideActions?: boolean;
  onClose: () => void;
  open: boolean;
  selectedLabel: string;
  title: string;
  topActionLabel?: string;
};

export const RelationshipSelectorDialog = ({
  applyDisabled = false,
  applyLabel,
  cancelLabel,
  children,
  clearLabel,
  closeLabel,
  onApply,
  onClear,
  onClose,
  hideActions = false,
  open,
  selectedLabel,
  title,
  topActionLabel,
}: Props) => {
  const theme = useTheme();
  const fullScreen = useMediaQuery(theme.breakpoints.down('sm'));

  return (
    <Dialog
      fullScreen={fullScreen}
      fullWidth
      maxWidth="sm"
      onClose={onClose}
      open={open}
      scroll="paper"
      slotProps={{
        paper: {
          sx: {
            height: { sm: 680 },
            maxHeight: { sm: 'calc(100dvh - 64px)' },
          },
        },
      }}
    >
      <DialogTitle>
        <Stack direction="row" sx={{ alignItems: 'center', gap: 1 }}>
          <Box sx={{ flexGrow: 1, minWidth: 0 }}>
            <Typography component="span" variant="h6">
              {title}
            </Typography>
            <Typography
              color="text.secondary"
              sx={{ display: 'block' }}
              variant="body2"
            >
              {selectedLabel}
            </Typography>
          </Box>
          {topActionLabel && (
            <Button onClick={onApply} size="small" variant="contained">
              {topActionLabel}
            </Button>
          )}
          <IconButton aria-label={closeLabel} onClick={onClose}>
            <CloseIcon />
          </IconButton>
        </Stack>
      </DialogTitle>
      <DialogContent dividers>{children}</DialogContent>
      {!hideActions && (
        <DialogActions sx={{ justifyContent: 'space-between' }}>
          <Button color="inherit" onClick={onClear}>
            {clearLabel}
          </Button>
          <Stack direction="row" spacing={1}>
            <Button color="inherit" onClick={onClose}>
              {cancelLabel}
            </Button>
            <Button
              disabled={applyDisabled}
              onClick={onApply}
              variant="contained"
            >
              {applyLabel}
            </Button>
          </Stack>
        </DialogActions>
      )}
    </Dialog>
  );
};
