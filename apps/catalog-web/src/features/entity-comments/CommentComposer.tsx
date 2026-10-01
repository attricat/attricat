import { useForm, useStore } from '@tanstack/react-form';
import { useMutation } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiRequestError } from '../../api/request';
import { draftEditors } from '../drafts/constants';
import { DraftRestoreDialog } from '../drafts/DraftRestoreDialog';
import { useEditorDraft } from '../drafts/useEditorDraft';
import { createComment, updateComment } from './api';
import { CommentMarkdown } from './CommentMarkdown';
import { maxCommentLength } from './constants';
import {
  commentDraftSchema,
  commentInputSchema,
  type EntityComment,
} from './schemas';

export const CommentComposer = ({
  entityId,
  comment,
  disabled = false,
  onSaved,
  onCancel,
  onReload,
}: {
  entityId: string;
  comment?: EntityComment;
  disabled?: boolean;
  onSaved: () => void;
  onCancel?: () => void;
  onReload?: () => Promise<void>;
}) => {
  const { t } = useTranslation();
  const inputId = useId();
  const [preview, setPreview] = useState(false);
  const mutation = useMutation({
    mutationFn: (body: string) =>
      comment
        ? updateComment(entityId, comment.id, comment.revision, body)
        : createComment(entityId, body),
    retry: false,
    onSuccess: () => {
      draft.clear();
      onSaved();
    },
  });
  const forbidden =
    mutation.error instanceof ApiRequestError &&
    [401, 403, 404].includes(mutation.error.status);
  const conflict =
    mutation.error instanceof ApiRequestError && mutation.error.status === 409;
  const locked = disabled || mutation.isPending || forbidden || conflict;
  const form = useForm({
    defaultValues: { body: comment?.body ?? '' },
    onSubmit: ({ value }) => {
      if (!locked && commentInputSchema.safeParse(value).success)
        mutation.mutate(value.body);
    },
  });
  const value = useStore(form.store, (state) => state.values);
  const valid = commentInputSchema.safeParse(value).success;
  const draft = useEditorDraft({
    editor: draftEditors.entityComment,
    resource: [entityId, comment?.id ?? 'new'],
    source: comment ? String(comment.revision) : null,
    ready: true,
    dirty: value.body !== (comment?.body ?? ''),
    value,
    schema: commentDraftSchema,
  });
  return (
    <Box
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
      sx={{ maxWidth: 720 }}
    >
      <Stack spacing={2}>
        <form.Field name="body">
          {(field) => (
            <TextField
              id={inputId}
              label={t(comment ? 'comments.editComment' : 'comments.comment')}
              required
              multiline
              minRows={3}
              fullWidth
              disabled={disabled || mutation.isPending || forbidden}
              value={field.state.value}
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              error={field.state.meta.isTouched && !valid}
              helperText={
                field.state.meta.isTouched && !valid
                  ? t('comments.validation', { max: maxCommentLength })
                  : t('comments.markdownHint', {
                      count: [...value.body].length,
                      max: maxCommentLength,
                    })
              }
            />
          )}
        </form.Field>
        {preview && (
          <Box component="section" aria-label={t('comments.preview')}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              {t('comments.preview')}
            </Typography>
            <CommentMarkdown body={value.body} />
          </Box>
        )}
        {mutation.error && (
          <Alert severity="error">
            {conflict ? t('comments.conflict') : mutation.error.message}
          </Alert>
        )}
        {conflict && onReload && (
          <Button type="button" onClick={() => void onReload()}>
            {t('comments.reload')}
          </Button>
        )}
        <Stack direction="row" spacing={1}>
          <Button
            type="submit"
            variant="contained"
            disabled={
              locked ||
              !valid ||
              (Boolean(comment) && value.body === comment?.body)
            }
          >
            {t(comment ? 'comments.save' : 'comments.post')}
          </Button>
          <Button
            type="button"
            aria-pressed={preview}
            onClick={() => setPreview(!preview)}
          >
            {t('comments.preview')}
          </Button>
          {onCancel && (
            <Button
              type="button"
              disabled={mutation.isPending}
              onClick={() => {
                draft.clear();
                onCancel();
              }}
            >
              {t('common.cancel')}
            </Button>
          )}
        </Stack>
      </Stack>
      <DraftRestoreDialog
        draft={draft.pending}
        onDiscard={draft.discard}
        onRestore={() => {
          const restored = draft.restore();
          if (restored) form.setFieldValue('body', restored.body);
        }}
      />
    </Box>
  );
};
