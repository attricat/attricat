import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
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
import {
  CircleAlertIcon,
  CircleCheckIcon,
  CircleDashedIcon,
  PencilIcon,
  PlusIcon,
  Trash2Icon,
} from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../app/theme';
import { ActionsHeaderCell } from '../../components/ActionsHeaderCell';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { CopyIconButton } from '../../components/CopyIconButton';
import { EmptyState } from '../../components/EmptyState';
import { EmptyValue } from '../../components/EmptyValue';
import { compactIconSize } from '../../components/iconSizes';
import { LabeledIconButton } from '../../components/LabeledIconButton';
import { LoadingState } from '../../components/LoadingState';
import { StatusChip, type StatusChipStyle } from '../../components/StatusChip';
import { ExtensionSecretIcon } from '../../components/systemIcons';
import { useToast } from '../../components/useToast';
import { Timestamp } from '../../time/Timestamp';
import { ExtensionSecretDialog } from './ExtensionSecretDialog';
import { deleteExtensionSecret } from './managementApi';
import { extensionSecretsOptions } from './queryOptions';
import type { ExtensionSecret } from './schemas';

/** How a secret stands for its extension. */
const secretStatuses = {
  set: 'set',
  /** Declared as required and not set: the extension cannot do its work. */
  missing: 'missing',
  /** Declared as optional and not set. */
  unset: 'unset',
} as const;
type SecretStatus = (typeof secretStatuses)[keyof typeof secretStatuses];

const secretStatusChips = {
  [secretStatuses.set]: { color: 'success', icon: CircleCheckIcon },
  [secretStatuses.missing]: { color: 'warning', icon: CircleAlertIcon },
  [secretStatuses.unset]: { color: 'default', icon: CircleDashedIcon },
} as const satisfies Record<SecretStatus, StatusChipStyle>;

const secretStatus = (secret: ExtensionSecret): SecretStatus => {
  if (secret.set) return secretStatuses.set;
  return secret.required ? secretStatuses.missing : secretStatuses.unset;
};

/**
 * Which dialog is open: adding a secret the manifest does not declare, or
 * setting the value of the named one, replacing a value it already has.
 */
type Editing = { name?: string; replacing: boolean } | undefined;

const SecretNameCell = ({ secret }: { secret: ExtensionSecret }) => {
  const { t } = useTranslation();
  return (
    <Stack spacing={0.25}>
      {secret.title && <Typography variant="body2">{secret.title}</Typography>}
      <Stack direction="row" spacing={0.5} sx={{ alignItems: 'center' }}>
        <Typography
          color={secret.title ? 'text.secondary' : undefined}
          component="code"
          sx={{ fontFamily: monoFontFamily, overflowWrap: 'anywhere' }}
          variant="body2"
        >
          {secret.name}
        </Typography>
        <CopyIconButton
          label={t('extensions.copySecretName', { name: secret.name })}
          value={secret.name}
        />
      </Stack>
      {secret.description && (
        <Typography color="text.secondary" variant="caption">
          {secret.description}
        </Typography>
      )}
    </Stack>
  );
};

/**
 * The secrets of one installed extension, such as API keys or webhook signing
 * keys, which only its component receives while it runs. The list shows the
 * secrets its manifest declares and any others set for it; values are
 * write-only, so it shows names and when each was set.
 */
export const ExtensionSecretsSection = ({
  canManage,
  extensionId,
}: {
  canManage: boolean;
  extensionId: string;
}) => {
  const { t } = useTranslation();
  const { show } = useToast();
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState<Editing>();
  // Kept after the dialog closes so its title does not empty while it fades.
  const [deleting, setDeleting] = useState<{ name: string; open: boolean }>();
  const secrets = useQuery(extensionSecretsOptions(extensionId));
  const names = useMemo(
    () => new Set(secrets.data?.map((secret) => secret.name)),
    [secrets.data],
  );
  const refresh = () =>
    queryClient.invalidateQueries({
      queryKey: extensionSecretsOptions(extensionId).queryKey,
    });
  const remove = useMutation({
    mutationFn: deleteExtensionSecret,
    onSuccess: async (_, { name }) => {
      setDeleting({ name, open: false });
      show({
        message: t('extensions.secretDeleted', { name }),
        severity: 'success',
      });
      await refresh();
    },
  });

  return (
    <Paper sx={{ p: 4 }}>
      <Stack direction="row" sx={{ alignItems: 'center', mb: 1 }}>
        <Typography component="h2" sx={{ flex: 1 }} variant="h6">
          {t('extensions.secrets')}
        </Typography>
        {canManage && (
          <Button
            disabled={!secrets.data}
            onClick={() => setEditing({ replacing: false })}
            startIcon={<PlusIcon />}
          >
            {t('extensions.addSecret')}
          </Button>
        )}
      </Stack>
      <Typography color="text.secondary" sx={{ mb: 1 }} variant="body2">
        {t('extensions.secretsDescription')}
      </Typography>
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
          <Table aria-label={t('extensions.secrets')} size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('extensions.secretName')}</TableCell>
                <TableCell>{t('extensions.secretStatus')}</TableCell>
                <TableCell>{t('extensions.secretUpdated')}</TableCell>
                {canManage && <ActionsHeaderCell />}
              </TableRow>
            </TableHead>
            <TableBody>
              {secrets.data?.map((secret) => {
                const status = secretStatus(secret);
                return (
                  <TableRow key={secret.name}>
                    <TableCell>
                      <SecretNameCell secret={secret} />
                    </TableCell>
                    <TableCell>
                      <StatusChip
                        label={t(`extensions.secretStatuses.${status}`)}
                        size="small"
                        style={secretStatusChips[status]}
                      />
                    </TableCell>
                    <TableCell>
                      {secret.updated_at ? (
                        <Timestamp style="dateTime" value={secret.updated_at} />
                      ) : (
                        <EmptyValue />
                      )}
                    </TableCell>
                    {canManage && (
                      <TableCell align="right" sx={{ whiteSpace: 'nowrap' }}>
                        {secret.set ? (
                          <>
                            <LabeledIconButton
                              label={t('extensions.replaceSecretNamed', {
                                name: secret.name,
                              })}
                              onClick={() =>
                                setEditing({
                                  name: secret.name,
                                  replacing: true,
                                })
                              }
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
                          </>
                        ) : (
                          <Button
                            aria-label={t('extensions.setSecretNamed', {
                              name: secret.name,
                            })}
                            onClick={() =>
                              setEditing({
                                name: secret.name,
                                replacing: false,
                              })
                            }
                            size="small"
                          >
                            {t('extensions.setSecretValue')}
                          </Button>
                        )}
                      </TableCell>
                    )}
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </TableContainer>
      )}
      {editing && (
        <ExtensionSecretDialog
          existingNames={names}
          extensionId={extensionId}
          name={editing.name}
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
        onConfirm={() =>
          deleting && remove.mutate({ extensionId, name: deleting.name })
        }
        open={deleting?.open ?? false}
        title={t('extensions.deleteSecretTitle', { name: deleting?.name })}
      >
        {t('extensions.deleteSecretExplanation')}
      </ConfirmDialog>
    </Paper>
  );
};
