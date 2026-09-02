import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Chip,
  Divider,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  decideApproval,
  getConversation,
  listApprovals,
  listMessages,
  listRuns,
  sendMessage,
} from './api';
import { agentQueryKeys } from './query-keys';

const displayContent = (content: unknown) =>
  typeof content === 'string' ? content : JSON.stringify(content, null, 2);
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
  const queryClient = useQueryClient();
  const [content, setContent] = useState('');
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
    mutationFn: () => sendMessage(conversationId, content),
    onSuccess: () => {
      setContent('');
      invalidate();
    },
  });
  const decide = useMutation({
    mutationFn: ({ id, approved }: { id: string; approved: boolean }) =>
      decideApproval(id, approved),
    onSuccess: invalidate,
  });

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
        setStreamError('Live updates disconnected; reconnecting…');
      return source;
    });
    return () => sources.forEach((source) => source.close());
  }, [queryClient, runs.data]); // EventSource reconnects with Last-Event-ID while a run remains active.

  return (
    <PageContainer maxWidth="md">
      <PageHeader
        actions={
          <Button component={Link} to="/agents/schedules" variant="outlined">
            Schedules
          </Button>
        }
        eyebrow="Agent conversation"
        title={conversation.data?.title ?? 'Conversation'}
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
            <Typography
              component="pre"
              sx={{ fontFamily: 'inherit', m: 0, whiteSpace: 'pre-wrap' }}
            >
              {displayContent(message.content)}
            </Typography>
          </Paper>
        ))}
        {messages.isPending && <Typography>Loading conversation...</Typography>}
      </Stack>
      {approvals.data?.map((call) => (
        <Paper
          key={call.id}
          sx={{ border: 1, borderColor: 'warning.main', mt: 3, p: 2 }}
        >
          <Typography variant="h6">
            Approval needed: {call.tool_name}
          </Typography>
          {call.change_summary && (
            <Typography sx={{ mt: 1 }}>{call.change_summary}</Typography>
          )}
          <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
            Proposed input
          </Typography>
          <Box
            component="pre"
            sx={{ bgcolor: 'action.hover', overflow: 'auto', p: 1 }}
          >
            {JSON.stringify(call.arguments, null, 2)}
          </Box>
          <Stack direction="row" spacing={1}>
            <Button
              color="success"
              disabled={decide.isPending}
              onClick={() => decide.mutate({ id: call.id, approved: true })}
              variant="contained"
            >
              Approve
            </Button>
            <Button
              color="error"
              disabled={decide.isPending}
              onClick={() => decide.mutate({ id: call.id, approved: false })}
              variant="outlined"
            >
              Reject
            </Button>
          </Stack>
        </Paper>
      ))}
      <Paper sx={{ mt: 3, p: 2 }}>
        <Typography variant="h6">Run status</Typography>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mt: 1 }}>
          {runs.data?.map((run) => (
            <Chip
              color={statusColor(run.status)}
              key={run.id}
              label={`${run.origin}: ${run.status}`}
            />
          ))}
        </Box>
        {runs.data
          ?.filter((run) => run.error_message)
          .map((run) => (
            <Alert key={`${run.id}-error`} severity="error" sx={{ mt: 1 }}>
              {run.error_message}
            </Alert>
          ))}
      </Paper>
      <Divider sx={{ my: 3 }} />
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          if (content.trim()) send.mutate();
        }}
      >
        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2}>
          <TextField
            fullWidth
            label="Message"
            multiline
            onChange={(event) => setContent(event.target.value)}
            placeholder="Ask the agent to help with your catalog…"
            value={content}
          />
          <Button
            disabled={!content.trim() || send.isPending}
            type="submit"
            variant="contained"
          >
            Send
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
