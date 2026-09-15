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

type DialogActionsConfig = {
  applyDisabled?: boolean;
  applyLabel: string;
  cancelLabel: string;
  clearLabel: string;
  onApply: () => void;
  onClear: () => void;
};

type Props = {
  actions?: DialogActionsConfig;
  children: ReactNode;
  closeLabel: string;
  onClose: () => void;
  open: boolean;
  selectedLabel: string;
  title: string;
  topAction?: {
    label: string;
    onClick: () => void;
  };
};

export const RelationshipSelectorDialog = ({
  actions,
  children,
  closeLabel,
  onClose,
  open,
  selectedLabel,
  title,
  topAction,
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
          {topAction && (
            <Button
              onClick={topAction.onClick}
              size="small"
              variant="contained"
            >
              {topAction.label}
            </Button>
          )}
          <IconButton aria-label={closeLabel} onClick={onClose}>
            <CloseIcon />
          </IconButton>
        </Stack>
      </DialogTitle>
      <DialogContent dividers>{children}</DialogContent>
      {actions && (
        <DialogActions sx={{ justifyContent: 'space-between' }}>
          <Button color="inherit" onClick={actions.onClear}>
            {actions.clearLabel}
          </Button>
          <Stack direction="row" spacing={1}>
            <Button color="inherit" onClick={onClose}>
              {actions.cancelLabel}
            </Button>
            <Button
              disabled={actions.applyDisabled}
              onClick={actions.onApply}
              variant="contained"
            >
              {actions.applyLabel}
            </Button>
          </Stack>
        </DialogActions>
      )}
    </Dialog>
  );
};
