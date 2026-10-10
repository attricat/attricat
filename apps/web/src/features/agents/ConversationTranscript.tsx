import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, Box, CircularProgress, Stack, Typography } from '@mui/material';
import { ApprovalCard } from './ApprovalCard';
import { loadingProgressSize, transcriptEndScrollMargin } from './constants';
import type { AgentRun, AgentToolCall, ConversationMessage } from './schemas';
import { ThinkingIndicator } from './ThinkingIndicator';
import { TranscriptMessage } from './TranscriptMessage';

export type ConversationTranscriptProps = {
  approvals: AgentToolCall[] | undefined;
  isDecidingApproval: boolean;
  isLoadingMessages: boolean;
  isThinking: boolean | undefined;
  latestRun: AgentRun | undefined;
  messages: ConversationMessage[] | undefined;
  onDecideApproval: (approvalId: string, approved: boolean) => void;
  onApplyDraft?: (fields: Record<string, string>) => void;
  getDraftValues?: () => Record<string, string>;
};

export const ConversationTranscript = ({
  approvals,
  isDecidingApproval,
  isLoadingMessages,
  isThinking,
  latestRun,
  messages,
  onDecideApproval,
  onApplyDraft,
  getDraftValues,
}: ConversationTranscriptProps) => {
  const { t } = useTranslation();
  const conversationEnd = useRef<HTMLDivElement>(null);

  useEffect(() => {
    conversationEnd.current?.scrollIntoView({
      behavior: 'smooth',
      block: 'end',
    });
  }, [approvals?.length, isThinking, messages?.length]);

  return (
    <Stack spacing={4} sx={{ flexGrow: 1, mx: 'auto', py: 4, width: '100%' }}>
      {messages?.map((message) => (
        <TranscriptMessage
          getDraftValues={getDraftValues}
          key={message.id}
          message={message}
          onApplyDraft={onApplyDraft}
        />
      ))}
      {isLoadingMessages && (
        <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
          <CircularProgress enableTrackSlot size={loadingProgressSize} />
          <Typography color="text.secondary">
            {t('agents.loadingConversation')}
          </Typography>
        </Stack>
      )}
      {isThinking && <ThinkingIndicator />}
      {latestRun?.error_message && (
        <Alert severity="error">
          <Typography component="pre" sx={{ m: 0, whiteSpace: 'pre-wrap' }}>
            {latestRun.error_code
              ? `${latestRun.error_code}: ${latestRun.error_message}`
              : latestRun.error_message}
          </Typography>
        </Alert>
      )}
      {approvals?.map((call) => (
        <ApprovalCard
          call={call}
          isDeciding={isDecidingApproval}
          key={call.id}
          onDecide={onDecideApproval}
        />
      ))}
      <Box
        ref={conversationEnd}
        sx={{ scrollMarginBottom: transcriptEndScrollMargin }}
      />
    </Stack>
  );
};
