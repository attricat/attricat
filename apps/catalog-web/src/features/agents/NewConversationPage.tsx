import AttachFileOutlinedIcon from '@mui/icons-material/AttachFileOutlined';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import { useForm } from '@tanstack/react-form';
import { useRef, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  IconButton,
  Paper,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { uploadConversationFiles } from '../files/api';
import { createConversation, sendMessage } from './api';
import { conversationTitleFromFirstMessage } from './conversationTitle';
import { agentQueryKeys } from './queryKeys';

export const NewConversationPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const form = useForm({ defaultValues: { content: '' } });
  const [attachments, setAttachments] = useState<File[]>([]);
  const [uploadedAttachmentIds, setUploadedAttachmentIds] = useState<string[]>(
    [],
  );
  const [conversationId, setConversationId] = useState<string | null>(null);
  const attachmentInput = useRef<HTMLInputElement>(null);
  const start = useMutation({
    meta: { toast: false },
    mutationFn: async ({
      content,
      files,
      previousAttachmentIds,
      existingConversationId,
    }: {
      content: string;
      files: File[];
      previousAttachmentIds: string[];
      existingConversationId: string | null;
    }) => {
      let id = existingConversationId;
      if (!id) {
        const conversation = await createConversation(
          conversationTitleFromFirstMessage(content),
        );
        id = conversation.id;
        setConversationId(id);
      }
      let attachmentIds = previousAttachmentIds;
      if (files.length > 0 && attachmentIds.length === 0) {
        const uploaded = await uploadConversationFiles(id, files);
        attachmentIds = uploaded.files.map((file) => file.id);
        setUploadedAttachmentIds(attachmentIds);
      }
      await sendMessage(id, content, attachmentIds);
      return id;
    },
    onSuccess: async (id) => {
      await queryClient.invalidateQueries({ queryKey: agentQueryKeys.all() });
      await navigate({
        to: '/agents/$conversationId',
        params: { conversationId: id },
      });
    },
  });
  const submit = () => {
    if (
      (form.state.values.content.trim() || attachments.length) &&
      !start.isPending
    ) {
      start.mutate({
        content: form.state.values.content,
        files: attachments,
        previousAttachmentIds: uploadedAttachmentIds,
        existingConversationId: conversationId,
      });
    }
  };

  return (
    <PageContainer>
      <Box sx={{ maxWidth: 760, mx: 'auto' }}>
        <PageHeader
          description={t('agents.newConversationDescription')}
          title={t('agents.newAgentConversation')}
          titleVariant="h3"
        />
        <Paper
          component="form"
          elevation={0}
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
          sx={{
            border: 1,
            borderColor: 'divider',
            borderRadius: 3,
            mt: 4,
            p: 0.75,
          }}
        >
          <form.Field name="content">
            {(field) => (
              <TextField
                disabled={start.isPending}
                fullWidth
                hiddenLabel
                maxRows={8}
                minRows={3}
                multiline
                onChange={(event) => field.handleChange(event.target.value)}
                onKeyDown={(event) => {
                  if (
                    event.key === 'Enter' &&
                    !event.shiftKey &&
                    !event.nativeEvent.isComposing
                  ) {
                    event.preventDefault();
                    submit();
                  }
                }}
                placeholder={t('agents.messagePlaceholder')}
                slotProps={{ htmlInput: { 'aria-label': t('agents.message') } }}
                sx={{ '& .MuiOutlinedInput-notchedOutline': { border: 0 } }}
                value={field.state.value}
              />
            )}
          </form.Field>
          <input
            disabled={start.isPending}
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
            <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', px: 1 }}>
              {attachments.map((file) => (
                <Chip
                  key={`${file.name}:${file.size}:${file.lastModified}`}
                  label={file.name}
                  onDelete={
                    start.isPending
                      ? undefined
                      : () => {
                          setAttachments((current) =>
                            current.filter((item) => item !== file),
                          );
                          setUploadedAttachmentIds([]);
                        }
                  }
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
                disabled={start.isPending}
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
                    (!form.state.values.content.trim() &&
                      !attachments.length) ||
                    start.isPending
                  }
                  type="submit"
                >
                  {start.isPending ? (
                    <CircularProgress enableTrackSlot size={20} />
                  ) : (
                    <ArrowUpwardIcon />
                  )}
                </IconButton>
              </span>
            </Tooltip>
          </Stack>
        </Paper>
        {start.isError && (
          <Alert
            action={
              <Button disabled={start.isPending} onClick={submit} size="small">
                {t('agents.retry')}
              </Button>
            }
            severity="error"
            sx={{ mt: 2 }}
          >
            {t('agents.messageNotSent')}: {start.error.message}
          </Alert>
        )}
      </Box>
    </PageContainer>
  );
};
