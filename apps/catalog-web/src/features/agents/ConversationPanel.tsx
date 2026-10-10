import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Box } from '@mui/material';
import { decideApproval } from './api';
import { smartFillRecordForm } from '../records/api';
import { thinkingRunStatuses } from './constants';
import { ConversationComposer } from './ConversationComposer';
import { ConversationTranscript } from './ConversationTranscript';
import {
  conversationApprovalsOptions,
  conversationMessagesOptions,
  conversationRunsOptions,
  conversationPollInterval,
  invalidateConversation,
} from './queryOptions';
import { useConversationLiveUpdates } from './useConversationLiveUpdates';

export type DraftContext = {
  recordId: string;
  contextId: string | null;
  defaultContextId: string | null;
  getDraftValues: () => Record<string, string>;
  onApply: (fields: Record<string, string>) => void;
};

export const ConversationPanel = ({
  conversationId,
  draft,
}: {
  conversationId: string;
  draft?: DraftContext;
}) => {
  const client = useQueryClient();
  const [isSending, setIsSending] = useState(false);
  const runs = useQuery({
    ...conversationRunsOptions(conversationId),
    // Discover newly started runs even when the existing stream is healthy.
    refetchInterval: (query) =>
      conversationPollInterval(query.state.data, false, isSending),
  });
  const live = useConversationLiveUpdates(conversationId, runs.data);
  const interval = conversationPollInterval(
    runs.data,
    live.connected,
    isSending,
  );
  const messages = useQuery({
    ...conversationMessagesOptions(conversationId),
    refetchInterval: interval,
  });
  const approvals = useQuery({
    ...conversationApprovalsOptions(conversationId),
    refetchInterval: interval,
  });
  const invalidate = () => void invalidateConversation(client, conversationId);
  const decide = useMutation({
    mutationFn: ({ id, approved }: { id: string; approved: boolean }) =>
      decideApproval(id, approved),
    onSuccess: invalidate,
  });
  return (
    <Box
      sx={{
        display: 'flex',
        flexDirection: 'column',
        flexGrow: 1,
        minHeight: 0,
      }}
    >
      {(messages.error || runs.error || approvals.error) && (
        <Alert severity="error">
          {(messages.error ?? runs.error ?? approvals.error)?.message}
        </Alert>
      )}
      {live.error && <Alert severity="info">{live.error}</Alert>}
      <Box sx={{ flexGrow: 1, overflowY: 'auto' }}>
        <ConversationTranscript
          approvals={approvals.data}
          isDecidingApproval={decide.isPending}
          isLoadingMessages={messages.isPending}
          isThinking={
            isSending ||
            runs.data?.some((run) => thinkingRunStatuses.includes(run.status))
          }
          latestRun={runs.data?.[0]}
          messages={messages.data}
          onDecideApproval={(id, approved) => decide.mutate({ id, approved })}
          onApplyDraft={draft?.onApply}
          getDraftValues={draft?.getDraftValues}
        />
      </Box>
      <ConversationComposer
        conversationId={conversationId}
        onSendingChange={setIsSending}
        onSent={invalidate}
        sendDraft={
          draft
            ? (content, id, attachmentIds) =>
                smartFillRecordForm({
                  record_id: draft.recordId,
                  context_id: draft.contextId,
                  is_default_context:
                    draft.contextId === draft.defaultContextId,
                  content,
                  conversation_id: id,
                  draft_values: draft.getDraftValues(),
                  attachment_ids: attachmentIds,
                })
            : undefined
        }
      />
    </Box>
  );
};
