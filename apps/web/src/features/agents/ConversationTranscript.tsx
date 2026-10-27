import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, Box, CircularProgress, Stack, Typography } from '@mui/material';
import { ApprovalCard } from './ApprovalCard';
import type {
  BulkProgress,
  AgentRun,
  AgentToolCall,
  ConversationMessage,
} from './schemas';
import { BulkEditProgress } from './BulkEditProgress';
import { transcriptEndScrollMargin } from './constants';
import { ThinkingIndicator } from './ThinkingIndicator';
import { groupToolActivity } from './toolActivity';
import { ToolActivityGroup } from './ToolActivityGroup';
import { TranscriptMessage } from './TranscriptMessage';
import { progressSizes } from '../../components/iconSizes';

type ConversationTranscriptProps = {
  approvals: AgentToolCall[] | undefined;
  bulkProgress?: BulkProgress[];
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
  bulkProgress,
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
      {groupToolActivity(messages ?? []).map((group) =>
        Array.isArray(group) ? (
          <ToolActivityGroup key={group[0].id} messages={group} />
        ) : (
          <TranscriptMessage
            getDraftValues={getDraftValues}
            key={group.id}
            message={group}
            onApplyDraft={onApplyDraft}
          />
        ),
      )}
      {isLoadingMessages && (
        <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
          <CircularProgress enableTrackSlot size={progressSizes.standard} />
          <Typography color="text.secondary">
            {t('agents.loadingConversation')}
          </Typography>
        </Stack>
      )}
      {bulkProgress?.map((progress) => (
        <BulkEditProgress key={progress.tool_call_id} progress={progress} />
      ))}
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
