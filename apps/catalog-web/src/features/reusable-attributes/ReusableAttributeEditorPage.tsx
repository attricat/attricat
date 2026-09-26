import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Box, Button, Paper, Stack, Typography } from '@mui/material';
import { useState } from 'react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  createReusableAttribute,
  createReusableAttributeRevision,
  listReusableAttributes,
} from './api';
import { TomlEditor } from '../../components/TomlEditor';
import { reusableAttributeQueryKeys } from './query-keys';
import { latestReusableAttributeRevisions } from './latest-revisions';

const newAttributeDefinition = `code = "new_attribute"
name = "New attribute"
value_type = "string"
`;

const definitionValue = (definition: string, key: string) =>
  definition.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, 'm'))?.[1];

const DefinitionPreview = ({ definition }: { definition: string }) => {
  const code = definitionValue(definition, 'code') ?? '—';
  const name = definitionValue(definition, 'name') ?? 'Untitled attribute';
  const valueType = definitionValue(definition, 'value_type') ?? 'string';
  return (
    <Paper sx={{ p: 2 }} variant="outlined">
      <Typography gutterBottom variant="subtitle2">
        Preview
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
  const definition =
    editedDefinition ?? attribute?.definition ?? newAttributeDefinition;
  const save = useMutation({
    mutationFn: (submittedDefinition: string) =>
      attribute
        ? createReusableAttributeRevision(attribute.definition_id, {
            definition: submittedDefinition,
          })
        : createReusableAttribute({ definition: submittedDefinition }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.root(),
      });
      navigate({ to: '/manage/reusable-attributes' });
    },
  });

  if (definitionId && attributes.isPending) {
    return (
      <PageContainer>
        <Typography>Loading reusable attribute…</Typography>
      </PageContainer>
    );
  }

  if (definitionId && !attribute) {
    return (
      <PageContainer>
        <Alert severity="error">Reusable attribute not found.</Alert>
      </PageContainer>
    );
  }

  return (
    <PageContainer>
      <Stack spacing={3}>
        <PageHeader
          description="Namespace is derived from the active workspace and cannot be set in the definition."
          title={
            attribute ? `Edit ${attribute.name}` : 'New reusable attribute'
          }
          actions={
            <Stack direction="row" spacing={1}>
              <Button
                disabled={save.isPending}
                onClick={() => navigate({ to: '/manage/reusable-attributes' })}
              >
                Cancel
              </Button>
              <Button
                disabled={save.isPending}
                onClick={() => save.mutate(definition)}
                variant="contained"
              >
                {attribute ? 'Save revision' : 'Create attribute'}
              </Button>
            </Stack>
          }
        />
        {save.error && <Alert severity="error">{save.error.message}</Alert>}
        <Box
          sx={{
            display: 'grid',
            gap: 2,
            gridTemplateColumns: { lg: 'minmax(0, 1fr) 300px' },
          }}
        >
          <TomlEditor
            height="calc(100vh - 280px)"
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
