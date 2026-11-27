import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { PencilIcon, PlusIcon, Trash2Icon } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../app/theme';
import { ActionsHeaderCell } from '../../components/ActionsHeaderCell';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { CopyIconButton } from '../../components/CopyIconButton';
import { EmptyState } from '../../components/EmptyState';
import { compactIconSize } from '../../components/iconSizes';
import { LabeledIconButton } from '../../components/LabeledIconButton';
import { LoadingState } from '../../components/LoadingState';
import { ExtensionSecretIcon } from '../../components/systemIcons';
import { useToast } from '../../components/useToast';
import { Timestamp } from '../../time/Timestamp';
import { capabilities } from '../auth/capabilities';
import { sessionOptions } from '../auth/queryOptions';
import { useCan } from '../auth/useCan';
import { ExtensionSecretDialog } from './ExtensionSecretDialog';
import {
  deleteWorkspaceExtensionSecret,
  workspaceExtensionSecrets,
} from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';

/** Which dialog is open: adding a secret, or replacing the named one's value. */
type Editing = { replacing?: string } | undefined;

/**
 * Named workspace secrets that extensions with `secrets.read` receive while
 * they run, such as an API key for an external service. Values are
 * write-only: the list shows names and when each was set.
 */
export const ExtensionSecretsPage = () => {
  const { t } = useTranslation();
  const { show } = useToast();
  const queryClient = useQueryClient();
  const session = useQuery(sessionOptions());
  const canManage = useCan(capabilities.extensionsManage);
  const [editing, setEditing] = useState<Editing>();
  // Kept after the dialog closes so its title does not empty while it fades.
  const [deleting, setDeleting] = useState<{ name: string; open: boolean }>();
  const secrets = useQuery({
    queryKey: extensionManagementQueryKeys.secrets(),
    queryFn: workspaceExtensionSecrets,
    enabled: canManage,
  });
  const names = useMemo(
    () => new Set(secrets.data?.map((secret) => secret.name)),
    [secrets.data],
  );
  const refresh = () =>
    queryClient.invalidateQueries({
      queryKey: extensionManagementQueryKeys.secrets(),
    });
  const remove = useMutation({
    // The page shows its own message.
    meta: { toast: false },
    mutationFn: deleteWorkspaceExtensionSecret,
    onSuccess: async (_, name) => {
      setDeleting({ name, open: false });
      show({
        message: t('extensions.secretDeleted', { name }),
        severity: 'success',
      });
      await refresh();
    },
  });

  if (session.data && !canManage) {
    return (
      <Alert severity="error">
        {t(`auth.notAuthorized.${capabilities.extensionsManage}`)}
      </Alert>
    );
  }

  return (
    <Paper sx={{ p: 2.5 }}>
      <Stack
        direction="row"
        spacing={2}
        sx={{ alignItems: 'flex-start', mb: 2 }}
      >
        <Box sx={{ flex: 1 }}>
          <Typography component="h2" variant="h5">
            {t('extensions.secrets')}
          </Typography>
          <Typography color="text.secondary">
            {t('extensions.secretsDescription')}
          </Typography>
        </Box>
        <Button
          disabled={!secrets.data}
          onClick={() => setEditing({})}
          startIcon={<PlusIcon />}
        >
          {t('extensions.addSecret')}
        </Button>
      </Stack>
      <ApiErrorAlert error={secrets.error} sx={{ mb: 2 }} />
      {secrets.isPending && (
        <LoadingState>{t('extensions.loadingSecrets')}</LoadingState>
      )}
      {secrets.data?.length === 0 && (
        <EmptyState
          description={t('extensions.noSecretsDescription')}
          icon={ExtensionSecretIcon}
          title={t('extensions.noSecrets')}
        />
      )}
      {Boolean(secrets.data?.length) && (
        <TableContainer>
          <Table aria-label={t('extensions.secrets')}>
            <TableHead>
              <TableRow>
                <TableCell>{t('extensions.secretName')}</TableCell>
                <TableCell>{t('extensions.secretCreated')}</TableCell>
                <TableCell>{t('extensions.secretUpdated')}</TableCell>
                <ActionsHeaderCell />
              </TableRow>
            </TableHead>
            <TableBody>
              {secrets.data?.map((secret) => (
                <TableRow key={secret.name}>
                  <TableCell>
                    <Stack
                      direction="row"
                      spacing={0.5}
                      sx={{ alignItems: 'center' }}
                    >
                      <Typography
                        component="code"
                        sx={{
                          fontFamily: monoFontFamily,
                          overflowWrap: 'anywhere',
                        }}
                        variant="body2"
                      >
                        {secret.name}
                      </Typography>
                      <CopyIconButton
                        label={t('extensions.copySecretName', {
                          name: secret.name,
                        })}
                        value={secret.name}
                      />
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Timestamp style="dateTime" value={secret.created_at} />
                  </TableCell>
                  <TableCell>
                    <Timestamp style="dateTime" value={secret.updated_at} />
                  </TableCell>
                  <TableCell align="right" sx={{ whiteSpace: 'nowrap' }}>
                    <LabeledIconButton
                      label={t('extensions.replaceSecretNamed', {
                        name: secret.name,
                      })}
                      onClick={() => setEditing({ replacing: secret.name })}
                      size="small"
                    >
                      <PencilIcon size={compactIconSize} />
                    </LabeledIconButton>
                    <LabeledIconButton
                      color="error"
                      label={t('extensions.deleteSecretNamed', {
                        name: secret.name,
                      })}
                      onClick={() => {
                        remove.reset();
                        setDeleting({ name: secret.name, open: true });
                      }}
                      size="small"
                    >
                      <Trash2Icon size={compactIconSize} />
                    </LabeledIconButton>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
      {editing && (
        <ExtensionSecretDialog
          existingNames={names}
          onClose={() => setEditing(undefined)}
          onSaved={(name) => {
            show({
              message: editing.replacing
                ? t('extensions.secretReplaced', { name })
                : t('extensions.secretAdded', { name }),
              severity: 'success',
            });
            setEditing(undefined);
            void refresh();
          }}
          replacing={editing.replacing}
        />
      )}
      <ConfirmDialog
        confirmLabel={t('extensions.deleteSecret')}
        destructive
        error={remove.error}
        isPending={remove.isPending}
        onClose={() =>
          setDeleting((current) => current && { ...current, open: false })
        }
        onConfirm={() => deleting && remove.mutate(deleting.name)}
        open={deleting?.open ?? false}
        title={t('extensions.deleteSecretTitle', { name: deleting?.name })}
      >
        {t('extensions.deleteSecretExplanation')}
      </ConfirmDialog>
    </Paper>
  );
};
