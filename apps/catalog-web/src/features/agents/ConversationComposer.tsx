import AttachFileOutlinedIcon from '@mui/icons-material/AttachFileOutlined';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import { useForm } from '@tanstack/react-form';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
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
import { uploadConversationFiles } from '../files/api';
import { sendMessage } from './api';

export type ConversationComposerProps = {
  conversationId: string;
  onSendingChange: (isSending: boolean) => void;
  onSent: () => void;
};

export const ConversationComposer = ({
  conversationId,
  onSendingChange,
  onSent,
}: ConversationComposerProps) => {
  const { t } = useTranslation();
  const form = useForm({ defaultValues: { content: '' } });
  const [attachments, setAttachments] = useState<File[]>([]);
  const [uploadedAttachmentIds, setUploadedAttachmentIds] = useState<string[]>(
    [],
  );
  const attachmentInput = useRef<HTMLInputElement>(null);
  const send = useMutation({
    meta: { toast: false },
    mutationFn: async ({
      content,
      files,
      previousAttachmentIds,
    }: {
      content: string;
      files: File[];
      previousAttachmentIds: string[];
    }) => {
      let attachmentIds = previousAttachmentIds;
      if (files.length > 0 && attachmentIds.length === 0) {
        const uploaded = await uploadConversationFiles(conversationId, files);
        attachmentIds = uploaded.files.map((file) => file.id);
        setUploadedAttachmentIds(attachmentIds);
      }
      return sendMessage(conversationId, content, attachmentIds);
    },
    onMutate: () => onSendingChange(true),
    onSuccess: () => {
      form.reset();
      setAttachments([]);
      setUploadedAttachmentIds([]);
      onSent();
    },
    onSettled: () => onSendingChange(false),
  });

  const submitMessage = () => {
    if (
      (form.state.values.content.trim() || attachments.length) &&
      !send.isPending
    ) {
      send.mutate({
        content: form.state.values.content,
        files: attachments,
        previousAttachmentIds: uploadedAttachmentIds,
      });
    }
  };

  return (
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
        <form.Field name="content">
          {(field) => (
            <TextField
              disabled={send.isPending}
              fullWidth
              hiddenLabel
              slotProps={{ htmlInput: { 'aria-label': t('agents.message') } }}
              maxRows={8}
              minRows={1}
              multiline
              onChange={(event) => field.handleChange(event.target.value)}
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
              value={field.state.value}
            />
          )}
        </form.Field>
        <input
          disabled={send.isPending}
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
                  send.isPending
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
              disabled={send.isPending}
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
                  (!form.state.values.content.trim() && !attachments.length) ||
                  send.isPending
                }
                type="submit"
              >
                {send.isPending ? (
                  <CircularProgress enableTrackSlot size={20} />
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
  );
};
