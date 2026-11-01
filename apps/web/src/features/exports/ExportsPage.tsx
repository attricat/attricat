import { EmptyState } from '../../components/EmptyState';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Chip,
  FormControlLabel,
  Paper,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';
import { useAccess, useCan } from '../auth/useCan';
import {
  updatePublicationChannel,
  type PublicationChannel,
  type PublicationChannelChecks,
} from './api';
import { ruleDefinitionsOptions } from '../rules/queryOptions';
import { ChannelChecksEditor } from './ChannelChecksEditor';
import { EXPORT_TABLE_COLUMN_COUNT } from './constants';
import { exportQueryKeys } from './queryKeys';
import { ContextIcon, ExportIcon } from '../../components/systemIcons';
import { capabilities, accessTargetKinds } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { contextListOptions } from '../contexts/queryOptions';
import { publicationChannelsOptions } from './queryOptions';
import { LoadingState } from '../../components/LoadingState';

export const ExportsPage = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const contexts = useQuery(contextListOptions());
  const channelAccess = useAccess(
    capabilities.contextsWrite,
    accessTargetKinds.context,
    contexts.data?.map((context) => context.id) ?? [],
  );
  const channels = useQuery(publicationChannelsOptions());
  const canReadRules = useCan(capabilities.rulesRead);
  // Rule codes are suggestions only; a reader without rule access can type them.
  const rules = useQuery({
    ...ruleDefinitionsOptions(),
    enabled: canReadRules,
  });
  const ruleCodes = [
    ...new Set(rules.data?.map((rule) => rule.code) ?? []),
  ].sort();
  const updateChannel = useMutation({
    mutationFn: ({
      contextId,
      enabled,
      checks,
    }: {
      contextId: string;
      enabled: boolean;
      checks?: PublicationChannelChecks;
    }) => updatePublicationChannel(contextId, enabled, checks),
    onSuccess: (channel) => {
      client.setQueryData<PublicationChannel[]>(
        exportQueryKeys.channels(),
        (current) => [
          ...(current ?? []).filter(
            (item) => item.context_id !== channel.context_id,
          ),
          channel,
        ],
      );
    },
  });
  const channelsByContextId = new Map(
    channels.data?.map((channel) => [channel.context_id, channel]) ?? [],
  );

  return (
    <PageContainer>
      <PageHeader icon={ExportIcon} title={t('exports.title')} />
      <Typography color="text.secondary" sx={{ mt: 1 }}>
        {t('exports.description')}
      </Typography>
      {contexts.data &&
        !channelAccess.isPending &&
        !contexts.data.some((context) => channelAccess.can(context.id)) && (
          // Without context write access every control below is disabled.
          <Typography color="text.secondary" sx={{ mt: 2 }} variant="body2">
            {t('exports.readOnly')}
          </Typography>
        )}
      <ApiErrorAlert
        error={contexts.error ?? channels.error ?? updateChannel.error}
        sx={{ mt: 3 }}
      />
      {(contexts.isPending || channels.isPending) && (
        <LoadingState sx={{ mt: 3 }}>{t('exports.loading')}</LoadingState>
      )}
      {contexts.data && channels.data && (
        <TableContainer component={Paper} sx={{ mt: 3 }}>
          <Table>
            <TableHead>
              <TableRow>
                <TableCell>{t('exports.context')}</TableCell>
                <TableCell>{t('exports.channel')}</TableCell>
                <TableCell>{t('exports.publicationChecks')}</TableCell>
                <TableCell>{t('exports.actions')}</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {contexts.data.map((context) => {
                const channel = channelsByContextId.get(context.id);
                const enabled = channel?.enabled ?? false;
                const canEdit = channelAccess.can(context.id);
                return (
                  <TableRow key={context.id}>
                    <TableCell>{context.code}</TableCell>
                    <TableCell>
                      <FormControlLabel
                        control={
                          <Switch
                            checked={enabled}
                            disabled={!canEdit || updateChannel.isPending}
                            onChange={(_, checked) =>
                              updateChannel.mutate({
                                contextId: context.id,
                                enabled: checked,
                              })
                            }
                          />
                        }
                        label={
                          <Chip
                            color={enabled ? 'success' : 'default'}
                            label={
                              enabled
                                ? t('exports.enabled')
                                : t('exports.disabled')
                            }
                            size="small"
                          />
                        }
                      />
                    </TableCell>
                    <TableCell>
                      <ChannelChecksEditor
                        disabled={!canEdit || updateChannel.isPending}
                        onChange={(checks) =>
                          updateChannel.mutate({
                            contextId: context.id,
                            enabled,
                            checks,
                          })
                        }
                        requireValidRecord={
                          channel?.require_valid_record ?? false
                        }
                        requiredRuleCodes={channel?.required_rule_codes ?? []}
                        ruleCodes={ruleCodes}
                      />
                    </TableCell>
                    <TableCell>
                      <RouterButton search={{ context: context.code }} to="/">
                        {t('exports.openInExplore')}
                      </RouterButton>
                    </TableCell>
                  </TableRow>
                );
              })}
              {!contexts.data.length && (
                <TableRow>
                  <TableCell colSpan={EXPORT_TABLE_COLUMN_COUNT}>
                    <EmptyState
                      icon={ContextIcon}
                      title={t('exports.noContexts')}
                    />
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </TableContainer>
      )}
    </PageContainer>
  );
};
