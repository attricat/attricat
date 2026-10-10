import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Chip,
  FormControlLabel,
  Paper,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/queryKeys';
import {
  listPublicationChannels,
  updatePublicationChannel,
  type PublicationChannel,
  type PublicationChannelChecks,
} from './api';
import { ruleDefinitionsOptions } from '../rules/queryOptions';
import { ChannelChecksEditor } from './ChannelChecksEditor';
import { EXPORT_TABLE_COLUMN_COUNT } from './constants';
import { exportQueryKeys } from './queryKeys';
import { ExportIcon } from '../../components/systemIcons';

export const ExportsPage = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const channels = useQuery({
    queryKey: exportQueryKeys.channels(),
    queryFn: listPublicationChannels,
  });
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  // Rule codes are suggestions only; a reader without rule access can type them.
  const rules = useQuery({
    ...ruleDefinitionsOptions(),
    enabled: session.data?.capabilities?.rules_read === true,
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
      {(contexts.isError || channels.isError || updateChannel.isError) && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {contexts.error?.message ??
            channels.error?.message ??
            updateChannel.error?.message}
        </Alert>
      )}
      {(contexts.isPending || channels.isPending) && (
        <Typography sx={{ mt: 3 }}>{t('exports.loading')}</Typography>
      )}
      {contexts.data && channels.data && (
        <Paper sx={{ mt: 3 }}>
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
                return (
                  <TableRow key={context.id}>
                    <TableCell>{context.code}</TableCell>
                    <TableCell>
                      <FormControlLabel
                        control={
                          <Switch
                            checked={enabled}
                            disabled={updateChannel.isPending}
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
                        disabled={updateChannel.isPending}
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
                    {t('exports.noContexts')}
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </Paper>
      )}
    </PageContainer>
  );
};
