import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Box, Chip, Stack } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  decideApproval,
  getConversation,
  listApprovals,
  listMessages,
  listRuns,
} from './api';
import { ConversationComposer } from './ConversationComposer';
import { ConversationTranscript } from './ConversationTranscript';
import { agentQueryKeys } from './queryKeys';
import { useConversationLiveUpdates } from './useConversationLiveUpdates';

export type ConversationDetailPageProps = {
  conversationId: string;
};

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
}: ConversationDetailPageProps) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [isSending, setIsSending] = useState(false);
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
    void queryClient.invalidateQueries({ queryKey: agentQueryKeys.all() });
  const decide = useMutation({
    mutationFn: ({ id, approved }: { id: string; approved: boolean }) =>
      decideApproval(id, approved),
    onSuccess: invalidate,
  });
  const latestRun = runs.data?.[0];
  const isThinking =
    isSending ||
    runs.data?.some((run) => ['queued', 'running'].includes(run.status));
  const streamError = useConversationLiveUpdates(runs.data);

  return (
    <PageContainer maxWidth={false}>
      <Box
        sx={{
          display: 'flex',
          flexDirection: 'column',
          minHeight: { md: 'calc(100dvh - 48px)' },
          mx: 'auto',
          width: '100%',
        }}
      >
        <PageHeader
          actions={
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              {latestRun && latestRun.status !== 'completed' && (
                <Chip
                  color={statusColor(latestRun.status)}
                  label={`${latestRun.origin}: ${latestRun.status}`}
                  size="small"
                />
              )}
            </Stack>
          }
          eyebrow={t('agents.agentConversation')}
          title={conversation.data?.title ?? t('agents.conversation')}
          titleVariant="h3"
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
        <ConversationTranscript
          approvals={approvals.data}
          isDecidingApproval={decide.isPending}
          isLoadingMessages={messages.isPending}
          isThinking={isThinking}
          latestRun={latestRun}
          messages={messages.data}
          onDecideApproval={(id, approved) => decide.mutate({ id, approved })}
        />
        <ConversationComposer
          conversationId={conversationId}
          onSendingChange={setIsSending}
          onSent={invalidate}
        />
      </Box>
    </PageContainer>
  );
};
