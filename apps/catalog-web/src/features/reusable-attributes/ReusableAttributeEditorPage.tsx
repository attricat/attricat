import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Box, Button, Paper, Stack, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  createReusableAttribute,
  createReusableAttributeRevision,
  listReusableAttributes,
} from './api';
import { definitionKinds } from '../definition-editor/constants';
import { DefinitionEditor } from '../definition-editor/DefinitionEditor';
import { draftEditors } from '../drafts/constants';
import { DraftRestoreDialog } from '../drafts/DraftRestoreDialog';
import { definitionDraftSchema } from '../drafts/schemas';
import { useEditorDraft } from '../drafts/useEditorDraft';
import { reusableAttributeQueryKeys } from './queryKeys';
import {
  DEFAULT_VALUE_TYPE,
  EDITOR_HEIGHT,
  EMPTY_VALUE_PLACEHOLDER,
  NEW_ATTRIBUTE_DEFINITION,
  PREVIEW_COLUMN_WIDTH,
} from './constants';
import { latestReusableAttributeRevisions } from './latestRevisions';
import { ReusableAttributeIcon } from '../../components/systemIcons';

const definitionValue = (definition: string, key: string) =>
  definition.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, 'm'))?.[1];

const DefinitionPreview = ({ definition }: { definition: string }) => {
  const { t } = useTranslation();
  const code = definitionValue(definition, 'code') ?? EMPTY_VALUE_PLACEHOLDER;
  const name =
    definitionValue(definition, 'name') ??
    t('reusableAttributes.editor.untitled');
  const valueType =
    definitionValue(definition, 'value_type') ?? DEFAULT_VALUE_TYPE;
  return (
    <Paper sx={{ p: 2 }} variant="outlined">
      <Typography gutterBottom variant="subtitle2">
        {t('reusableAttributes.editor.preview')}
      </Typography>
      <Typography color="text.secondary" variant="caption">
        {valueType} · {code}
      </Typography>
      <Box
        component="input"
        disabled
        sx={{
          border: 1,
          borderColor: 'divider',
          borderRadius: 1,
          boxSizing: 'border-box',
          display: 'block',
          mt: 1,
          p: 1.5,
          width: '100%',
        }}
        value={name}
      />
    </Paper>
  );
};

export const ReusableAttributeEditorPage = ({
  definitionId,
}: {
  definitionId?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const attributes = useQuery({
    queryKey: reusableAttributeQueryKeys.definitions(true),
    queryFn: ({ signal }) => listReusableAttributes(true, signal),
  });
  const attribute = latestReusableAttributeRevisions(
    attributes.data ?? [],
  ).find((item) => item.definition_id === definitionId);
  const [editedDefinition, setEditedDefinition] = useState<string>();
  const initialDefinition = attribute?.definition ?? NEW_ATTRIBUTE_DEFINITION;
  const definition = editedDefinition ?? initialDefinition;
  const draft = useEditorDraft({
    dirty: definition !== initialDefinition,
    editor: definitionId
      ? draftEditors.reusableAttributeRevision
      : draftEditors.reusableAttributeCreate,
    ready: !definitionId || Boolean(attribute),
    resource: definitionId ? [definitionId] : [],
    schema: definitionDraftSchema,
    source: attribute?.definition ?? null,
    value: definition,
  });
  const save = useMutation({
    mutationFn: (submittedDefinition: string) =>
      attribute
        ? createReusableAttributeRevision(attribute.definition_id, {
            definition: submittedDefinition,
          })
        : createReusableAttribute({ definition: submittedDefinition }),
    onSuccess: async () => {
      draft.clear();
      await queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.root(),
      });
      navigate({ to: '/manage/reusable-attributes' });
    },
  });

  if (definitionId && attributes.isPending) {
    return (
      <PageContainer>
        <Typography>{t('reusableAttributes.editor.loading')}</Typography>
      </PageContainer>
    );
  }

  if (definitionId && !attribute) {
    return (
      <PageContainer>
        <Alert severity="error">
          {t('reusableAttributes.editor.notFound')}
        </Alert>
      </PageContainer>
    );
  }

  return (
    <PageContainer>
      <Stack spacing={3}>
        <PageHeader
          description={t('reusableAttributes.editor.description')}
          icon={ReusableAttributeIcon}
          title={
            attribute
              ? t('reusableAttributes.editor.editTitle', {
                  name: attribute.name,
                })
              : t('reusableAttributes.editor.newTitle')
          }
          actions={
            <Stack direction="row" spacing={1}>
              <Button
                disabled={save.isPending}
                onClick={() => navigate({ to: '/manage/reusable-attributes' })}
              >
                {t('reusableAttributes.editor.cancel')}
              </Button>
              <Button
                disabled={save.isPending}
                onClick={() => save.mutate(definition)}
                variant="contained"
              >
                {attribute
                  ? t('reusableAttributes.editor.saveRevision')
                  : t('reusableAttributes.editor.createAttribute')}
              </Button>
            </Stack>
          }
        />
        <DraftRestoreDialog
          draft={draft.pending}
          onDiscard={draft.discard}
          onRestore={() => {
            const restored = draft.restore();
            if (restored !== undefined) setEditedDefinition(restored);
          }}
        />
        {save.error && <Alert severity="error">{save.error.message}</Alert>}
        <Box
          sx={{
            display: 'grid',
            gap: 2,
            gridTemplateColumns: {
              lg: `minmax(0, 1fr) ${PREVIEW_COLUMN_WIDTH}`,
            },
          }}
        >
          <DefinitionEditor
            kind={definitionKinds.reusableAttribute}
            height={EDITOR_HEIGHT}
            onChange={(value) => {
              if (!save.isPending) setEditedDefinition(value ?? '');
            }}
            readOnly={save.isPending}
            value={definition}
          />
          <DefinitionPreview definition={definition} />
        </Box>
      </Stack>
    </PageContainer>
  );
};
