import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Divider,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { fileDownloadUrl, uploadConversationFiles } from '../files/api';
import {
  decideApproval,
  getConversation,
  listApprovals,
  listMessages,
  listRuns,
  sendMessage,
} from './api';
import { ConversationMessageContent } from './ConversationMessageContent';
import { agentQueryKeys } from './query-keys';
const statusColor = (
  status: string,
): 'default' | 'success' | 'error' | 'warning' | 'info' => {
  if (status === 'completed') return 'success';
  if (status === 'failed' || status === 'cancelled') return 'error';
  if (status === 'awaiting_approval' || status === 'skipped') return 'warning';
  return status === 'queued' || status === 'running' ? 'info' : 'default';
};

export const ConversationDetailPage = ({
  conversationId,
}: {
  conversationId: string;
}) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [content, setContent] = useState('');
  const [attachments, setAttachments] = useState<File[]>([]);
  const attachmentInput = useRef<HTMLInputElement>(null);
  const [streamError, setStreamError] = useState<string | null>(null);
  const conversation = useQuery({
    queryKey: agentQueryKeys.conversation(conversationId),
    queryFn: () => getConversation(conversationId),
  });
  const messages = useQuery({
    queryKey: agentQueryKeys.messages(conversationId),
    queryFn: () => listMessages(conversationId),
    refetchInterval: 10_000,
  });
  const runs = useQuery({
    queryKey: agentQueryKeys.runs(conversationId),
    queryFn: () => listRuns(conversationId),
    refetchInterval: 10_000,
  });
  const approvals = useQuery({
    queryKey: agentQueryKeys.approvals(conversationId),
    queryFn: () => listApprovals(conversationId),
    refetchInterval: 10_000,
  });
  const invalidate = () =>
    void queryClient.invalidateQueries({ queryKey: agentQueryKeys.all });
  const send = useMutation({
    mutationFn: async () => {
      const uploaded = attachments.length
        ? await uploadConversationFiles(conversationId, attachments)
        : { files: [] };
      return sendMessage(
        conversationId,
        content,
        uploaded.files.map((file) => file.id),
      );
    },
    onSuccess: () => {
      setContent('');
      setAttachments([]);
      invalidate();
    },
  });
  const decide = useMutation({
    mutationFn: ({ id, approved }: { id: string; approved: boolean }) =>
      decideApproval(id, approved),
    onSuccess: invalidate,
  });
  const latestRun = runs.data?.[0];
  const submitMessage = () => {
    if ((content.trim() || attachments.length) && !send.isPending) {
      send.mutate();
    }
  };

  useEffect(() => {
    const activeRuns =
      runs.data?.filter((run) =>
        ['queued', 'running', 'awaiting_approval'].includes(run.status),
      ) ?? [];
    const sources = activeRuns.map((run) => {
      const source = new EventSource(`/api/agent/runs/${run.id}/events`);
      const update = () => {
        setStreamError(null);
        void queryClient.invalidateQueries({ queryKey: agentQueryKeys.all });
      };
      [
        'status',
        'message',
        'tool_call',
        'approval_required',
        'error',
        'terminal',
        'schedule_skipped',
      ].forEach((type) => source.addEventListener(type, update));
      source.onerror = () =>
        setStreamError(t('agents.liveUpdatesDisconnected'));
      return source;
    });
    return () => sources.forEach((source) => source.close());
  }, [queryClient, runs.data, t]); // EventSource reconnects with Last-Event-ID while a run remains active.

  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button component={Link} to="/agents/schedules" variant="outlined">
            {t('agents.schedules')}
          </Button>
        }
        eyebrow={t('agents.agentConversation')}
        title={conversation.data?.title ?? t('agents.conversation')}
      />
      {(conversation.isError ||
        messages.isError ||
        runs.isError ||
        approvals.isError) && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {conversation.error?.message ??
            messages.error?.message ??
            runs.error?.message ??
            approvals.error?.message}
        </Alert>
      )}
      {streamError && (
        <Alert severity="info" sx={{ mt: 2 }}>
          {streamError}
        </Alert>
      )}
      <Stack spacing={2} sx={{ mt: 3 }}>
        {messages.data?.map((message) => (
          <Paper
            key={message.id}
            sx={{
              alignSelf: message.role === 'user' ? 'flex-end' : 'stretch',
              bgcolor:
                message.role === 'user' ? 'primary.light' : 'background.paper',
              maxWidth: '90%',
              p: 2,
            }}
          >
            <Typography color="text.secondary" variant="caption">
              {message.role}
            </Typography>
            <ConversationMessageContent
              content={message.content}
              role={message.role}
            />
            {message.attachments.length > 0 && (
              <Stack direction="row" gap={1} sx={{ flexWrap: 'wrap', mt: 1 }}>
                {message.attachments.map((attachment) => (
                  <Chip
                    component="a"
                    clickable
                    href={fileDownloadUrl(attachment.id)}
                    key={attachment.id}
                    label={attachment.filename}
                    size="small"
                  />
                ))}
              </Stack>
            )}
          </Paper>
        ))}
        {messages.isPending && (
          <Typography>{t('agents.loadingConversation')}</Typography>
        )}
      </Stack>
      {approvals.data?.map((call) => (
        <Paper
          key={call.id}
          sx={{ border: 1, borderColor: 'warning.main', mt: 3, p: 2 }}
        >
          <Typography variant="h6">
            {t('agents.approvalNeeded', { tool: call.tool_name })}
          </Typography>
          {call.change_summary && (
            <Typography sx={{ mt: 1 }}>{call.change_summary}</Typography>
          )}
          <Accordion
            disableGutters
            elevation={0}
            sx={{ bgcolor: 'action.hover', mt: 1 }}
          >
            <AccordionSummary>{t('agents.showProposedInput')}</AccordionSummary>
            <AccordionDetails>
              <Box
                component="pre"
                sx={{ m: 0, overflow: 'auto', whiteSpace: 'pre-wrap' }}
              >
                {JSON.stringify(call.arguments, null, 2)}
              </Box>
            </AccordionDetails>
          </Accordion>
          <Stack direction="row" spacing={1}>
            <Button
              color="success"
              disabled={decide.isPending}
              onClick={() => decide.mutate({ id: call.id, approved: true })}
              variant="contained"
            >
              {t('agents.approve')}
            </Button>
            <Button
              color="error"
              disabled={decide.isPending}
              onClick={() => decide.mutate({ id: call.id, approved: false })}
              variant="outlined"
            >
              {t('agents.reject')}
            </Button>
          </Stack>
        </Paper>
      ))}
      <Paper sx={{ mt: 3, p: 2 }}>
        <Typography variant="h6">{t('agents.runStatus')}</Typography>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mt: 1 }}>
          {runs.isPending ? (
            <CircularProgress
              aria-label={t('agents.loadingRunStatus')}
              size={20}
            />
          ) : (
            latestRun && (
              <Chip
                color={statusColor(latestRun.status)}
                label={`${latestRun.origin}: ${latestRun.status}`}
              />
            )
          )}
        </Box>
        {latestRun?.error_message && (
          <Alert severity="error" sx={{ mt: 1 }}>
            <Typography component="pre" sx={{ m: 0, whiteSpace: 'pre-wrap' }}>
              {latestRun.error_code
                ? `${latestRun.error_code}: ${latestRun.error_message}`
                : latestRun.error_message}
            </Typography>
          </Alert>
        )}
      </Paper>
      <Divider sx={{ my: 3 }} />
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          submitMessage();
        }}
      >
        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2}>
          <Stack spacing={1} sx={{ flexGrow: 1 }}>
            <TextField
              fullWidth
              label={t('agents.message')}
              multiline
              onChange={(event) => setContent(event.target.value)}
              onKeyDown={(event) => {
                if (
                  event.key === 'Enter' &&
                  !event.shiftKey &&
                  !event.nativeEvent.isComposing
                ) {
                  event.preventDefault();
                  submitMessage();
                }
              }}
              placeholder={t('agents.messagePlaceholder')}
              value={content}
            />
            <input
              hidden
              multiple
              onChange={(event) => {
                setAttachments(Array.from(event.target.files ?? []));
                event.target.value = '';
              }}
              ref={attachmentInput}
              type="file"
            />
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              <Button onClick={() => attachmentInput.current?.click()}>
                {t('agents.addFiles')}
              </Button>
              {attachments.map((file) => (
                <Chip
                  key={`${file.name}:${file.size}:${file.lastModified}`}
                  label={file.name}
                  onDelete={() =>
                    setAttachments((current) =>
                      current.filter((item) => item !== file),
                    )
                  }
                  size="small"
                />
              ))}
            </Stack>
          </Stack>
          <Button
            disabled={
              (!content.trim() && !attachments.length) || send.isPending
            }
            type="submit"
            variant="contained"
          >
            {t('agents.send')}
          </Button>
        </Stack>
        {send.isError && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {send.error.message}
          </Alert>
        )}
      </Box>
    </PageContainer>
  );
};
