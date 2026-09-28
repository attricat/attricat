import { AgentIcon } from '../../components/systemIcons';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Alert,
  Avatar,
  Box,
  Button,
  Chip,
  CircularProgress,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import { fileDownloadUrl } from '../files/api';
import { ConversationMessageContent } from './ConversationMessageContent';
import { DraftProposal } from './DraftProposal';
import type { AgentRun, AgentToolCall, ConversationMessage } from './schemas';

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
      {messages?.map((message) => {
        const isUser = message.role === 'user';
        return (
          <Stack
            direction="row"
            key={message.id}
            spacing={1.5}
            sx={{
              alignItems: 'flex-start',
              alignSelf: isUser ? 'flex-end' : 'stretch',
              flexDirection: isUser ? 'row-reverse' : 'row',
              maxWidth: isUser ? { xs: '100%', sm: '80%' } : '100%',
            }}
          >
            <Avatar
              aria-label={isUser ? t('agents.you') : t('agents.assistant')}
              sx={{
                bgcolor: isUser ? 'text.primary' : 'primary.main',
                height: 30,
                mt: 0.25,
                width: 30,
              }}
            >
              {isUser ? (
                t('agents.you').slice(0, 1)
              ) : (
                <AgentIcon fontSize="small" />
              )}
            </Avatar>
            <Box
              sx={{
                bgcolor: isUser ? 'action.hover' : 'transparent',
                borderRadius: isUser ? 3 : 0,
                minWidth: 0,
                px: isUser ? 2 : 0,
                py: isUser ? 1.25 : 0,
                width: isUser ? 'fit-content' : '100%',
              }}
            >
              <Typography
                color="text.secondary"
                sx={{ display: 'block', mb: 0.5 }}
                variant="caption"
              >
                {isUser ? t('agents.you') : t('agents.assistant')}
              </Typography>
              <ConversationMessageContent
                content={message.content}
                messageRole={message.role}
              />
              {onApplyDraft &&
                message.role === 'assistant' &&
                typeof message.content === 'object' &&
                message.content !== null &&
                'draft_proposal' in message.content &&
                (() => {
                  const proposal = message.content.draft_proposal;
                  if (
                    !proposal ||
                    typeof proposal !== 'object' ||
                    !('fields' in proposal) ||
                    !proposal.fields ||
                    typeof proposal.fields !== 'object' ||
                    Array.isArray(proposal.fields)
                  )
                    return null;
                  const fields = Object.fromEntries(
                    Object.entries(proposal.fields).filter(
                      (entry): entry is [string, string] =>
                        typeof entry[1] === 'string',
                    ),
                  );
                  return (
                    <DraftProposal
                      proposal={{
                        fields,
                        baseValues:
                          'base_values' in proposal &&
                          typeof proposal.base_values === 'object' &&
                          proposal.base_values !== null &&
                          !Array.isArray(proposal.base_values)
                            ? Object.fromEntries(
                                Object.entries(proposal.base_values).filter(
                                  (entry): entry is [string, string | null] =>
                                    typeof entry[1] === 'string' ||
                                    entry[1] === null,
                                ),
                              )
                            : undefined,
                        explanation:
                          'explanation' in proposal &&
                          typeof proposal.explanation === 'string'
                            ? proposal.explanation
                            : '',
                      }}
                      onApply={onApplyDraft}
                      getDraftValues={getDraftValues}
                    />
                  );
                })()}
              {message.attachments.length > 0 && (
                <Stack
                  direction="row"
                  spacing={1}
                  sx={{ flexWrap: 'wrap', mt: 1 }}
                >
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
            </Box>
          </Stack>
        );
      })}
      {isLoadingMessages && (
        <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
          <CircularProgress enableTrackSlot size={18} />
          <Typography color="text.secondary">
            {t('agents.loadingConversation')}
          </Typography>
        </Stack>
      )}
      {isThinking && (
        <Stack
          aria-live="polite"
          direction="row"
          role="status"
          spacing={1.5}
          sx={{ alignItems: 'center' }}
        >
          <Avatar sx={{ bgcolor: 'primary.main', height: 30, width: 30 }}>
            <AgentIcon fontSize="small" />
          </Avatar>
          <Typography color="text.secondary" variant="body2">
            {t('agents.thinking')}
            <Box
              component="span"
              sx={{
                '@keyframes agent-thinking': {
                  '0%, 80%, 100%': { opacity: 0.25 },
                  '40%': { opacity: 1 },
                },
                '& span': {
                  animation: 'agent-thinking 1.4s infinite ease-in-out',
                  display: 'inline-block',
                  ml: 0.25,
                },
                '& span:nth-of-type(2)': { animationDelay: '0.16s' },
                '& span:nth-of-type(3)': { animationDelay: '0.32s' },
              }}
            >
              <span>•</span>
              <span>•</span>
              <span>•</span>
            </Box>
          </Typography>
        </Stack>
      )}
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
        <Paper
          key={call.id}
          sx={{ border: 1, borderColor: 'warning.main', p: 2.5 }}
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
            sx={{ bgcolor: 'action.hover', mt: 1.5 }}
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
          <Stack direction="row" spacing={1} sx={{ mt: 2 }}>
            <Button
              color="success"
              disabled={isDecidingApproval}
              onClick={() => onDecideApproval(call.id, true)}
              variant="contained"
            >
              {t('agents.approve')}
            </Button>
            <Button
              color="error"
              disabled={isDecidingApproval}
              onClick={() => onDecideApproval(call.id, false)}
              variant="outlined"
            >
              {t('agents.reject')}
            </Button>
          </Stack>
        </Paper>
      ))}
      <Box
        ref={conversationEnd}
        sx={{ scrollMarginBottom: { md: '8rem', xs: '10rem' } }}
      />
    </Stack>
  );
};
