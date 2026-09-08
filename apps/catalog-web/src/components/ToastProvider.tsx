import { Alert, Box, Stack } from '@mui/material';
import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  type ReactNode,
} from 'react';
import { ToastContext } from './toast-context';
import { reduceToasts, type Toast } from './toast-queue';
import { subscribeToToasts, type ToastOptions } from './toast';

const maximumVisibleToasts = 3;
let nextToastId = 0;

const ToastAlert = ({
  dismiss,
  toast,
}: {
  dismiss: (id: number) => void;
  toast: Toast;
}) => {
  useEffect(() => {
    const autoHideDuration = toast.autoHideDuration ?? 6000;
    if (autoHideDuration === null) return;

    const timer = window.setTimeout(() => dismiss(toast.id), autoHideDuration);
    return () => window.clearTimeout(timer);
  }, [dismiss, toast.autoHideDuration, toast.id]);

  return (
    <Box
      sx={{ maxWidth: 'calc(100vw - 32px)', position: 'relative', width: 480 }}
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
            WebkitLineClamp: 3,
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
          aria-label={`${toast.count} grouped notifications`}
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
            left: -10,
            lineHeight: 1,
            minWidth: 20,
            position: 'absolute',
            px: 0.75,
            py: 0.5,
            textAlign: 'center',
            top: -10,
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
  const [toasts, dispatch] = useReducer(reduceToasts, []);
  const show = useCallback((options: ToastOptions) => {
    dispatch({
      toast: {
        ...options,
        id: nextToastId++,
        severity: options.severity ?? 'info',
      },
      type: 'show',
    });
  }, []);
  const dismiss = useCallback((id: number) => {
    dispatch({ id, type: 'dismiss' });
  }, []);

  useEffect(() => subscribeToToasts(show), [show]);

  const value = useMemo(() => ({ dismiss, show }), [dismiss, show]);
  const visibleToasts = toasts.slice(0, maximumVisibleToasts);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <Stack
        aria-live="polite"
        spacing={1}
        sx={{
          bottom: 24,
          maxWidth: 'calc(100vw - 32px)',
          pointerEvents: 'none',
          position: 'fixed',
          right: 24,
          zIndex: (theme) => theme.zIndex.snackbar,
        }}
      >
        {visibleToasts.map((toast) => (
          <Box key={toast.id} sx={{ pointerEvents: 'auto' }}>
            <ToastAlert dismiss={dismiss} toast={toast} />
          </Box>
        ))}
      </Stack>
    </ToastContext.Provider>
  );
};
