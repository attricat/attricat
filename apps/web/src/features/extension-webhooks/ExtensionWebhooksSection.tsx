import {
  Button,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import {
  useIsMutating,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import {
  CircleAlertIcon,
  CircleCheckIcon,
  CircleDotIcon,
  CircleXIcon,
  PlusIcon,
  RotateCwIcon,
  Trash2Icon,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  isClientErrorStatus,
  isServerErrorStatus,
  isSuccessStatus,
} from '../../api/httpStatus';
import { ActionsHeaderCell } from '../../components/ActionsHeaderCell';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { EmptyState } from '../../components/EmptyState';
import { emptyValuePlaceholder } from '../../components/placeholders';
import { StatusChip, type StatusChipStyle } from '../../components/StatusChip';
import { ExtensionWebhookIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { deleteWebhookEndpoint, rotateWebhookEndpoint } from './api';
import { extensionWebhookQueryKeys } from './queryKeys';
import { webhookEndpointsOptions } from './queryOptions';
import type {
  IssuedWebhookEndpoint,
  ManifestWebhook,
  WebhookEndpoint,
} from './schemas';
import { WebhookEndpointDialog } from './WebhookEndpointDialog';
import { WebhookUrlDialog } from './WebhookUrlDialog';

const replyStatusChips = {
  success: { color: 'success', icon: CircleCheckIcon },
  refused: { color: 'warning', icon: CircleAlertIcon },
  failed: { color: 'error', icon: CircleXIcon },
  other: { color: 'default', icon: CircleDotIcon },
} as const satisfies Record<string, StatusChipStyle>;

/**
 * How the extension's last reply went: a 2xx accepted the request, a 4xx
 * refused it, and anything from 500 up is a failure.
 */
const replyStatusChip = (status: number) => {
  if (isSuccessStatus(status)) return replyStatusChips.success;
  if (isClientErrorStatus(status)) return replyStatusChips.refused;
  if (isServerErrorStatus(status)) return replyStatusChips.failed;
  return replyStatusChips.other;
};

const EndpointRow = ({
  canManage,
  endpoint,
  extensionId,
  onIssued,
  webhookTitle,
}: {
  canManage: boolean;
  endpoint: WebhookEndpoint;
  extensionId: string;
  onIssued: (issued: IssuedWebhookEndpoint) => void;
  webhookTitle: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const refresh = () =>
    client.invalidateQueries({
      queryKey: extensionWebhookQueryKeys.endpoints(extensionId),
    });
  const mutationKey = extensionWebhookQueryKeys.endpointMutations(
    extensionId,
    endpoint.endpoint,
  );
  const busy = useIsMutating({ mutationKey }) > 0;
  const [confirming, setConfirming] = useState<'rotate' | 'delete' | null>(
    null,
  );
  const rotate = useMutation({
    mutationKey,
    mutationFn: () => rotateWebhookEndpoint(extensionId, endpoint.endpoint),
    onSuccess: async (issued) => {
      setConfirming(null);
      onIssued(issued);
      await refresh();
    },
  });
  const remove = useMutation({
    mutationKey,
    mutationFn: () => deleteWebhookEndpoint(extensionId, endpoint.endpoint),
    onSuccess: async () => {
      await refresh();
      setConfirming(null);
    },
  });
  const close = () => {
    setConfirming(null);
    rotate.reset();
    remove.reset();
  };
  return (
    <TableRow>
      <TableCell sx={{ overflowWrap: 'anywhere' }}>
        {endpoint.endpoint}
      </TableCell>
      <TableCell>{webhookTitle}</TableCell>
      <TableCell>
        <Timestamp
          fallback={t('extensionWebhooks.neverRequested')}
          relative
          value={endpoint.last_request_at}
        />
      </TableCell>
      <TableCell>
        {endpoint.last_reply_status === null ? (
          emptyValuePlaceholder
        ) : (
          <StatusChip
            label={endpoint.last_reply_status}
            size="small"
            style={replyStatusChip(endpoint.last_reply_status)}
          />
        )}
      </TableCell>
      {canManage && (
        <TableCell>
          <Stack
            direction="row"
            spacing={1}
            sx={{ flexWrap: 'wrap', justifyContent: 'flex-end' }}
            useFlexGap
          >
            <Button
              disabled={busy}
              size="small"
              startIcon={<RotateCwIcon />}
              onClick={() => setConfirming('rotate')}
            >
              {t('extensionWebhooks.rotateUrl')}
            </Button>
            <Button
              color="error"
              disabled={busy}
              size="small"
              startIcon={<Trash2Icon />}
              onClick={() => setConfirming('delete')}
            >
              {t('extensionWebhooks.delete')}
            </Button>
          </Stack>
          <ConfirmDialog
            confirmLabel={t('extensionWebhooks.rotateUrl')}
            error={rotate.error}
            isPending={rotate.isPending}
            onClose={close}
            onConfirm={() => rotate.mutate()}
            open={confirming === 'rotate'}
            title={t('extensionWebhooks.rotateTitle', {
              name: endpoint.endpoint,
            })}
          >
            {t('extensionWebhooks.rotateExplanation')}
          </ConfirmDialog>
          <ConfirmDialog
            confirmLabel={t('extensionWebhooks.delete')}
            destructive
            error={remove.error}
            isPending={remove.isPending}
            onClose={close}
            onConfirm={() => remove.mutate()}
            open={confirming === 'delete'}
            title={t('extensionWebhooks.deleteTitle', {
              name: endpoint.endpoint,
            })}
          >
            {t('extensionWebhooks.deleteExplanation')}
          </ConfirmDialog>
        </TableCell>
      )}
    </TableRow>
  );
};

/**
 * Public endpoints external systems send webhooks to. Each URL carries a
 * secret token that only the create or rotate response shows; the extension
 * answers each request and checks its signature itself.
 */
export const ExtensionWebhooksSection = ({
  canManage,
  extensionId,
  webhooks,
}: {
  canManage: boolean;
  extensionId: string;
  webhooks: ManifestWebhook[];
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [creating, setCreating] = useState(false);
  const [issued, setIssued] = useState<IssuedWebhookEndpoint | null>(null);
  const endpoints = useQuery({
    ...webhookEndpointsOptions(extensionId),
    enabled: webhooks.length > 0,
  });
  if (!webhooks.length) return null;
  const webhookTitle = (id: string) =>
    webhooks.find((webhook) => webhook.id === id)?.title ?? id;
  return (
    <Paper sx={{ p: 4 }}>
      <Stack direction="row" sx={{ alignItems: 'center', mb: 1 }}>
        <Typography component="h2" sx={{ flex: 1 }} variant="h6">
          {t('extensionWebhooks.title')}
        </Typography>
        {canManage && (
          <Button startIcon={<PlusIcon />} onClick={() => setCreating(true)}>
            {t('extensionWebhooks.addEndpoint')}
          </Button>
        )}
      </Stack>
      <Typography color="text.secondary" sx={{ mb: 1 }} variant="body2">
        {t('extensionWebhooks.description')}
      </Typography>
      <ApiErrorAlert error={endpoints.error} />
      {endpoints.data?.length === 0 && (
        <EmptyState
          description={t('extensionWebhooks.noEndpointsDescription')}
          icon={ExtensionWebhookIcon}
          title={t('extensionWebhooks.noEndpoints')}
        />
      )}
      {!!endpoints.data?.length && (
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('extensionWebhooks.endpointName')}</TableCell>
              <TableCell>{t('extensionWebhooks.webhook')}</TableCell>
              <TableCell>{t('extensionWebhooks.lastRequest')}</TableCell>
              <TableCell>{t('extensionWebhooks.lastReply')}</TableCell>
              {canManage && <ActionsHeaderCell />}
            </TableRow>
          </TableHead>
          <TableBody>
            {endpoints.data.map((endpoint) => (
              <EndpointRow
                canManage={canManage}
                endpoint={endpoint}
                extensionId={extensionId}
                key={endpoint.id}
                onIssued={setIssued}
                webhookTitle={webhookTitle(endpoint.webhook)}
              />
            ))}
          </TableBody>
        </Table>
      )}
      {creating && (
        <WebhookEndpointDialog
          extensionId={extensionId}
          onClose={() => setCreating(false)}
          onCreated={(created) => {
            setCreating(false);
            setIssued(created);
            void client.invalidateQueries({
              queryKey: extensionWebhookQueryKeys.endpoints(extensionId),
            });
          }}
          webhooks={webhooks}
        />
      )}
      {issued && (
        <WebhookUrlDialog issued={issued} onClose={() => setIssued(null)} />
      )}
    </Paper>
  );
};
