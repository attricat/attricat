import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Box } from '@mui/material';
import { decideApproval, listApprovals, listMessages, listRuns } from './api';
import { smartFillEntityForm } from '../entities/api';
import { ConversationComposer } from './ConversationComposer';
import { ConversationTranscript } from './ConversationTranscript';
import { agentQueryKeys } from './queryKeys';
import { useConversationLiveUpdates } from './useConversationLiveUpdates';

export type DraftContext = {
  entityId: string;
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
    void client.invalidateQueries({ queryKey: agentQueryKeys.all() });
  const decide = useMutation({
    mutationFn: ({ id, approved }: { id: string; approved: boolean }) =>
      decideApproval(id, approved),
    onSuccess: invalidate,
  });
  const streamError = useConversationLiveUpdates(runs.data);
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
      {streamError && <Alert severity="info">{streamError}</Alert>}
      <Box sx={{ flexGrow: 1, overflowY: 'auto' }}>
        <ConversationTranscript
          approvals={approvals.data}
          isDecidingApproval={decide.isPending}
          isLoadingMessages={messages.isPending}
          isThinking={
            isSending ||
            runs.data?.some((run) => ['queued', 'running'].includes(run.status))
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
                smartFillEntityForm({
                  entity_id: draft.entityId,
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
