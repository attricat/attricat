import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Button,
  FormControl,
  InputLabel,
  MenuItem,
  Paper,
  Select,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  createSchedule,
  deleteSchedule,
  listConversations,
  listSchedules,
  runScheduleNow,
  updateSchedule,
} from './api';
import { agentQueryKeys } from './query-keys';

export const SchedulesPage = () => {
  const { i18n, t } = useTranslation();
  const formatDate = (value: string | null) =>
    value
      ? `${new Intl.DateTimeFormat(i18n.language, {
          dateStyle: 'medium',
          timeStyle: 'short',
          timeZone: 'UTC',
        }).format(new Date(value))} UTC`
      : t('agents.never');
  const queryClient = useQueryClient();
  const [conversationId, setConversationId] = useState('');
  const [cron, setCron] = useState('0 0 * * * *');
  const conversations = useQuery({
    queryKey: agentQueryKeys.conversations(),
    queryFn: listConversations,
  });
  const schedules = useQuery({
    queryKey: agentQueryKeys.schedules(),
    queryFn: () => listSchedules(),
    refetchInterval: 15_000,
  });
  const invalidate = () =>
    void queryClient.invalidateQueries({ queryKey: agentQueryKeys.all() });
  const create = useMutation({
    mutationFn: () => createSchedule(conversationId, cron),
    onSuccess: invalidate,
  });
  const update = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      updateSchedule(id, { enabled }),
    onSuccess: invalidate,
  });
  const runNow = useMutation({
    mutationFn: runScheduleNow,
    onSuccess: invalidate,
  });
  const remove = useMutation({
    mutationFn: deleteSchedule,
    onSuccess: invalidate,
  });
  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button component={Link} to="/agents" variant="outlined">
            {t('agents.conversations')}
          </Button>
        }
        description={t('agents.schedulesDescription')}
        title={t('agents.agentSchedules')}
      />
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          if (conversationId && cron.trim()) create.mutate();
        }}
        sx={{ mt: 3, p: 2 }}
      >
        <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}>
          <FormControl fullWidth>
            <InputLabel id="conversation-label">
              {t('agents.conversationLabel')}
            </InputLabel>
            <Select
              label={t('agents.conversationLabel')}
              labelId="conversation-label"
              onChange={(event) => setConversationId(event.target.value)}
              value={conversationId}
            >
              {conversations.data?.map((conversation) => (
                <MenuItem key={conversation.id} value={conversation.id}>
                  {conversation.title || t('agents.untitledConversation')}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
          <TextField
            fullWidth
            helperText={t('agents.cronHelp')}
            label={t('agents.cronExpression')}
            onChange={(event) => setCron(event.target.value)}
            value={cron}
          />
          <Button
            disabled={!conversationId || !cron.trim() || create.isPending}
            type="submit"
            variant="contained"
          >
            {t('agents.schedule')}
          </Button>
        </Stack>
        {create.isError && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {create.error.message}
          </Alert>
        )}
      </Paper>
      {schedules.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {schedules.error.message}
        </Alert>
      )}
      <Paper sx={{ mt: 3, overflowX: 'auto' }}>
        <Table>
          <TableHead>
            <TableRow>
              <TableCell>{t('agents.conversationLabel')}</TableCell>
              <TableCell>{t('agents.utcCron')}</TableCell>
              <TableCell>{t('agents.nextRun')}</TableCell>
              <TableCell>{t('agents.lastRun')}</TableCell>
              <TableCell>{t('agents.enabled')}</TableCell>
              <TableCell>{t('agents.actions')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {schedules.data?.map((schedule) => (
              <TableRow key={schedule.id}>
                <TableCell>
                  {conversations.data?.find(
                    (item) => item.id === schedule.conversation_id,
                  )?.title ?? schedule.conversation_id}
                </TableCell>
                <TableCell>{schedule.cron_expression}</TableCell>
                <TableCell>{formatDate(schedule.next_run_at)}</TableCell>
                <TableCell>{formatDate(schedule.last_run_at)}</TableCell>
                <TableCell>
                  <Switch
                    checked={schedule.enabled}
                    disabled={update.isPending}
                    slotProps={{
                      input: {
                        'aria-label': t('agents.enableSchedule', {
                          cron: schedule.cron_expression,
                        }),
                      },
                    }}
                    onChange={(event) =>
                      update.mutate({
                        id: schedule.id,
                        enabled: event.target.checked,
                      })
                    }
                  />
                </TableCell>
                <TableCell>
                  <Stack direction="row" spacing={1}>
                    <Button
                      disabled={runNow.isPending}
                      onClick={() => runNow.mutate(schedule.id)}
                      size="small"
                    >
                      {t('agents.runNow')}
                    </Button>
                    <Button
                      color="error"
                      disabled={remove.isPending}
                      onClick={() => remove.mutate(schedule.id)}
                      size="small"
                    >
                      {t('agents.delete')}
                    </Button>
                  </Stack>
                </TableCell>
              </TableRow>
            ))}
            {schedules.data?.length === 0 && (
              <TableRow>
                <TableCell colSpan={6}>
                  <Typography>{t('agents.noSchedules')}</Typography>
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </Paper>
      {(runNow.isError || update.isError || remove.isError) && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {runNow.error?.message ??
            update.error?.message ??
            remove.error?.message}
        </Alert>
      )}
    </PageContainer>
  );
};
