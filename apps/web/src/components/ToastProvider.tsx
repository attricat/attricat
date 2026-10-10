import { Alert, Box, Stack } from '@mui/material';
import { useEffect, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Toast } from './toastQueue';
import { useToastStore } from './toastStore';

const maximumVisibleToasts = 3;
const defaultToastAutoHideDuration = 6_000;
const toastWidth = 480;
const toastViewportMaxWidth = 'calc(100vw - 32px)';
const toastCountBadgeOffset = -10;
const toastCountBadgeMinWidth = 20;
const toastMessageMaxLines = 3;

const ToastAlert = ({
  dismiss,
  toast,
}: {
  dismiss: (id: number) => void;
  toast: Toast;
}) => {
  const { t } = useTranslation();
  useEffect(() => {
    const autoHideDuration =
      toast.autoHideDuration ?? defaultToastAutoHideDuration;
    if (autoHideDuration === null) return;

    const timer = window.setTimeout(() => dismiss(toast.id), autoHideDuration);
    return () => window.clearTimeout(timer);
  }, [dismiss, toast.autoHideDuration, toast.id]);

  return (
    <Box
      sx={{
        maxWidth: toastViewportMaxWidth,
        position: 'relative',
        width: toastWidth,
      }}
    >
      <Alert
        action={toast.action}
        onClose={() => dismiss(toast.id)}
        severity={toast.severity}
        sx={{
          width: '100%',
          '& .MuiAlert-message': { minWidth: 0, overflow: 'hidden' },
        }}
        variant="filled"
      >
        <Box
          component="span"
          sx={{
            WebkitBoxOrient: 'vertical',
            WebkitLineClamp: toastMessageMaxLines,
            display: '-webkit-box',
            overflow: 'hidden',
            overflowWrap: 'anywhere',
            textOverflow: 'ellipsis',
          }}
        >
          {toast.message}
        </Box>
      </Alert>
      {toast.count > 1 && (
        <Box
          aria-label={t('common.groupedNotifications', {
            count: toast.count,
          })}
          component="span"
          sx={{
            bgcolor: 'background.paper',
            border: '1px solid',
            borderColor: 'divider',
            borderRadius: '999px',
            boxShadow: 2,
            color: 'text.primary',
            fontSize: '0.75rem',
            fontWeight: 700,
            left: toastCountBadgeOffset,
            lineHeight: 1,
            minWidth: toastCountBadgeMinWidth,
            position: 'absolute',
            px: 0.75,
            py: 0.5,
            textAlign: 'center',
            top: toastCountBadgeOffset,
            zIndex: 1,
          }}
        >
          ×{toast.count}
        </Box>
      )}
    </Box>
  );
};

export const ToastProvider = ({ children }: { children: ReactNode }) => {
  const toasts = useToastStore((state) => state.toasts);
  const dismiss = useToastStore((state) => state.dismiss);
  const visibleToasts = toasts.slice(0, maximumVisibleToasts);

  return (
    <>
      {children}
      <Stack
        aria-live="polite"
        spacing={1}
        sx={{
          bottom: (theme) => theme.spacing(6),
          maxWidth: toastViewportMaxWidth,
          pointerEvents: 'none',
          position: 'fixed',
          right: (theme) => theme.spacing(6),
          zIndex: (theme) => theme.zIndex.snackbar,
        }}
      >
        {visibleToasts.map((toast) => (
          <Box key={toast.id} sx={{ pointerEvents: 'auto' }}>
            <ToastAlert dismiss={dismiss} toast={toast} />
          </Box>
        ))}
      </Stack>
    </>
  );
};
