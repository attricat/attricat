import AttachFileOutlinedIcon from '@mui/icons-material/AttachFileOutlined';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import SmartToyOutlinedIcon from '@mui/icons-material/SmartToyOutlined';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
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
  IconButton,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { fileDownloadUrl, uploadConversationFiles } from '../files/api';
import {
  agentRunEventsUrl,
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
  const [uploadedAttachmentIds, setUploadedAttachmentIds] = useState<string[]>(
    [],
  );
  const attachmentInput = useRef<HTMLInputElement>(null);
  const conversationEnd = useRef<HTMLDivElement>(null);
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
    meta: { toast: false },
    mutationFn: async () => {
      let attachmentIds = uploadedAttachmentIds;
      if (attachments.length > 0 && attachmentIds.length === 0) {
        const uploaded = await uploadConversationFiles(
          conversationId,
          attachments,
        );
        attachmentIds = uploaded.files.map((file) => file.id);
        setUploadedAttachmentIds(attachmentIds);
      }
      return sendMessage(conversationId, content, attachmentIds);
    },
    onSuccess: () => {
      setContent('');
      setAttachments([]);
      setUploadedAttachmentIds([]);
      invalidate();
    },
  });
  const decide = useMutation({
    mutationFn: ({ id, approved }: { id: string; approved: boolean }) =>
      decideApproval(id, approved),
    onSuccess: invalidate,
  });
  const latestRun = runs.data?.[0];
  const isThinking =
    send.isPending ||
    runs.data?.some((run) => ['queued', 'running'].includes(run.status));
  const submitMessage = () => {
    if ((content.trim() || attachments.length) && !send.isPending) {
      send.mutate();
    }
  };

  useEffect(() => {
    conversationEnd.current?.scrollIntoView({
      behavior: 'smooth',
      block: 'end',
    });
  }, [approvals.data?.length, isThinking, messages.data?.length]);

  useEffect(() => {
    const activeRuns =
      runs.data?.filter((run) =>
        ['queued', 'running', 'awaiting_approval'].includes(run.status),
      ) ?? [];
    const sources = activeRuns.map((run) => {
      const source = new EventSource(agentRunEventsUrl(run.id));
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
  }, [queryClient, runs.data, t]);

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
              <Button component={Link} to="/agents/schedules" variant="text">
                {t('agents.schedules')}
              </Button>
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
        <Stack
          spacing={4}
          sx={{ flexGrow: 1, mx: 'auto', py: 4, width: '100%' }}
        >
          {messages.data?.map((message) => {
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
                    <SmartToyOutlinedIcon fontSize="small" />
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
          {messages.isPending && (
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              <CircularProgress size={18} />
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
                <SmartToyOutlinedIcon fontSize="small" />
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
          {approvals.data?.map((call) => (
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
                <AccordionSummary>
                  {t('agents.showProposedInput')}
                </AccordionSummary>
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
                  disabled={decide.isPending}
                  onClick={() => decide.mutate({ id: call.id, approved: true })}
                  variant="contained"
                >
                  {t('agents.approve')}
                </Button>
                <Button
                  color="error"
                  disabled={decide.isPending}
                  onClick={() =>
                    decide.mutate({ id: call.id, approved: false })
                  }
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
        <Box
          component="form"
          onSubmit={(event) => {
            event.preventDefault();
            submitMessage();
          }}
          sx={{
            bgcolor: 'background.default',
            bottom: 0,
            mx: 'auto',
            pb: { xs: 1, md: 2 },
            position: 'sticky',
            pt: 2,
            width: '100%',
          }}
        >
          <Paper
            elevation={0}
            sx={{
              border: 1,
              borderColor: 'divider',
              borderRadius: 3,
              p: 0.75,
            }}
          >
            <TextField
              fullWidth
              hiddenLabel
              slotProps={{ htmlInput: { 'aria-label': t('agents.message') } }}
              maxRows={8}
              minRows={1}
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
              sx={{ '& .MuiOutlinedInput-notchedOutline': { border: 0 } }}
              value={content}
            />
            <input
              hidden
              multiple
              onChange={(event) => {
                setAttachments(Array.from(event.target.files ?? []));
                setUploadedAttachmentIds([]);
                event.target.value = '';
              }}
              ref={attachmentInput}
              type="file"
            />
            {attachments.length > 0 && (
              <Stack
                direction="row"
                spacing={1}
                sx={{ flexWrap: 'wrap', px: 1 }}
              >
                {attachments.map((file) => (
                  <Chip
                    key={`${file.name}:${file.size}:${file.lastModified}`}
                    label={file.name}
                    onDelete={() => {
                      setAttachments((current) =>
                        current.filter((item) => item !== file),
                      );
                      setUploadedAttachmentIds([]);
                    }}
                    size="small"
                  />
                ))}
              </Stack>
            )}
            <Stack
              direction="row"
              sx={{
                alignItems: 'center',
                justifyContent: 'space-between',
                mt: 0.5,
              }}
            >
              <Tooltip title={t('agents.addFiles')}>
                <IconButton
                  aria-label={t('agents.addFiles')}
                  onClick={() => attachmentInput.current?.click()}
                >
                  <AttachFileOutlinedIcon />
                </IconButton>
              </Tooltip>
              <Tooltip title={t('agents.send')}>
                <span>
                  <IconButton
                    aria-label={t('agents.send')}
                    color="primary"
                    disabled={
                      (!content.trim() && !attachments.length) || send.isPending
                    }
                    type="submit"
                  >
                    {send.isPending ? (
                      <CircularProgress size={20} />
                    ) : (
                      <ArrowUpwardIcon />
                    )}
                  </IconButton>
                </span>
              </Tooltip>
            </Stack>
          </Paper>
          {send.isError && (
            <Alert
              action={
                <Button
                  disabled={send.isPending}
                  onClick={submitMessage}
                  size="small"
                >
                  {t('agents.retry')}
                </Button>
              }
              severity="error"
              sx={{ mt: 1 }}
            >
              {t('agents.messageNotSent')}: {send.error.message}
            </Alert>
          )}
        </Box>
      </Box>
    </PageContainer>
  );
};
