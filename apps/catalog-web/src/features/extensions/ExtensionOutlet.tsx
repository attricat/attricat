import { Alert, Snackbar, Stack } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { z } from 'zod';
import { ExtensionFrame } from './ExtensionFrame';
import { getExtensionRuntime } from './api';
import { extensionQueryKeys } from './query-keys';

type Props = {
  outlet:
    | 'navigation'
    | 'entity_preview_panel'
    | 'blueprint_attribute_configuration'
    | 'entity_attribute_decoration'
    | 'entity_action';
  context?: Record<string, unknown>;
};

const notificationSchema = z
  .object({
    message: z.string().trim().min(1).max(512),
    severity: z.enum(['success', 'info', 'warning', 'error']).optional(),
  })
  .strict();

/** A fixed host-owned insertion point; extensions never choose a DOM selector. */
export const ExtensionOutlet = ({ outlet, context }: Props) => {
  const [notification, setNotification] = useState<{
    message: string;
    severity: 'success' | 'info' | 'warning' | 'error';
  }>();
  useEffect(() => {
    const receive = (event: Event) => {
      const parsed = notificationSchema.safeParse(
        (event as CustomEvent<unknown>).detail,
      );
      if (parsed.success)
        setNotification({
          message: parsed.data.message,
          severity: parsed.data.severity ?? 'info',
        });
    };
    window.addEventListener('catalog:extension-notify.v1', receive);
    return () =>
      window.removeEventListener('catalog:extension-notify.v1', receive);
  }, []);
  const runtime = useQuery({
    queryKey: extensionQueryKeys.runtime,
    queryFn: getExtensionRuntime,
    retry: false,
  });
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
          ?.filter((item) => item.kind === 'element' && item.outlet === outlet)
          .map((item) => (
            <ExtensionFrame
              contribution={item}
              context={context}
              key={`${item.extension_id}:${item.id}:${item.release_id}`}
            />
          ))}
      </Stack>
      <Snackbar
        autoHideDuration={6000}
        message={notification?.message}
        onClose={() => setNotification(undefined)}
        open={Boolean(notification)}
      >
        <Alert
          onClose={() => setNotification(undefined)}
          severity={notification?.severity ?? 'info'}
          variant="filled"
        >
          {notification?.message}
        </Alert>
      </Snackbar>
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
    retry: false,
  });
  if (runtime.isPending) return null;
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
