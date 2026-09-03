import { Editor, type Monaco } from '@monaco-editor/react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Card,
  CardActionArea,
  CardContent,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  Typography,
} from '@mui/material';
import { useEffect, useState } from 'react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  createBlueprint,
  createBlueprintRevision,
  getBlueprintRevision,
} from './api';
import { blueprintQueryKeys } from './query-keys';

const blueprintTemplates = [
  {
    definition: `format_version = 1
code = "new_blueprint"
name = "New blueprint"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[[attributes]]
code = "name"
value_type = "string"
`,
    description: 'A minimal entity with a name.',
    label: 'Basic entity',
  },
  {
    definition: `format_version = 1
code = "product"
name = "Product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name", "sku"]
separator = " / "

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "sku"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"

[[attributes]]
code = "description"
value_type = "string"

[[attributes]]
code = "product_images"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image"]
allowed_extensions = ["jpg", "jpeg", "png", "webp"]
max_bytes = 10485760
purposes = ["product_image"]
image_only = true
`,
    description:
      'A product with identifiers, price, description, and image uploads.',
    label: 'Product',
  },
  {
    definition: `format_version = 1
code = "seo"
name = "SEO"
kind = "mixin"

[[attributes]]
code = "meta_title"
value_type = "string"

[[attributes]]
code = "meta_description"
value_type = "string"
`,
    description: 'Reusable SEO fields for inclusion in entities.',
    label: 'SEO mixin',
  },
] as const;

const configureToml = (monaco: Monaco) => {
  if (
    monaco.languages
      .getLanguages()
      .some((language: { id: string }) => language.id === 'toml')
  )
    return;
  monaco.languages.register({ id: 'toml' });
  monaco.languages.setMonarchTokensProvider('toml', {
    tokenizer: {
      root: [
        [/^\s*#.*$/, 'comment'],
        [/\[[^\]]+\]/, 'keyword'],
        [/[A-Za-z0-9_-]+(?=\s*=)/, 'type.identifier'],
        [/"([^"\\]|\\.)*"|'([^'\\]|\\.)*'/, 'string'],
        [/\b(true|false)\b/, 'keyword'],
        [/-?\d+(\.\d+)?/, 'number'],
      ],
    },
  });
};

export const BlueprintEditorPage = ({
  blueprintId,
  sourceVersion,
}: {
  blueprintId?: string;
  sourceVersion?: number;
}) => {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const source = useQuery({
    queryKey: blueprintQueryKeys.revision(
      blueprintId ?? '',
      sourceVersion ?? 0,
    ),
    queryFn: () => getBlueprintRevision(blueprintId!, sourceVersion!),
    enabled: Boolean(blueprintId && sourceVersion),
  });
  const [templateIndex, setTemplateIndex] = useState(0);
  const [templateDialogOpen, setTemplateDialogOpen] = useState(!blueprintId);
  const initialDefinition =
    source.data?.blueprint.definition ??
    blueprintTemplates[templateIndex].definition;
  const [editedDefinition, setEditedDefinition] = useState<string | null>(null);
  const definition = editedDefinition ?? initialDefinition;
  const isDirty = definition !== initialDefinition;
  useEffect(() => {
    const warnBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!isDirty) return;
      event.preventDefault();
      event.returnValue = '';
    };
    window.addEventListener('beforeunload', warnBeforeUnload);
    return () => window.removeEventListener('beforeunload', warnBeforeUnload);
  }, [isDirty]);

  const save = useMutation({
    mutationFn: () =>
      blueprintId
        ? createBlueprintRevision(blueprintId, definition)
        : createBlueprint(definition),
    onSuccess: async ({ blueprint }) => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.catalogue(),
        }),
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.revisions(blueprint.id),
        }),
      ]);
      navigate({
        params: { blueprintId: blueprint.id },
        to: '/manage/blueprints/$blueprintId',
      });
    },
  });

  const selectTemplate = (index: number) => {
    if (isDirty && !window.confirm('Replace your unsaved blueprint changes?'))
      return;
    setTemplateIndex(index);
    setEditedDefinition(null);
    setTemplateDialogOpen(false);
  };

  const cancel = () => {
    if (isDirty && !window.confirm('Discard unsaved blueprint changes?'))
      return;
    navigate(
      blueprintId
        ? { params: { blueprintId }, to: '/manage/blueprints/$blueprintId' }
        : { to: '/manage/blueprints' },
    );
  };

  if (blueprintId && source.isPending) {
    return (
      <PageContainer>
        <Typography>Loading blueprint source...</Typography>
      </PageContainer>
    );
  }

  return (
    <PageContainer>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={2}
        sx={{ alignItems: { sm: 'center' }, justifyContent: 'space-between' }}
      >
        <PageHeader
          description={
            blueprintId
              ? `Creating a draft from version ${sourceVersion}. Publishing remains a separate step.`
              : 'Start a draft blueprint from the included valid TOML template.'
          }
          title={blueprintId ? 'New blueprint revision' : 'New blueprint'}
        />
        <Stack direction="row" spacing={1}>
          {!blueprintId && (
            <Button onClick={() => setTemplateDialogOpen(true)}>
              Examples
            </Button>
          )}
          {isDirty && <Chip color="warning" label="Unsaved changes" />}
          <Button disabled={save.isPending} onClick={cancel}>
            Cancel
          </Button>
          <Button
            disabled={
              save.isPending ||
              source.isError ||
              Boolean(blueprintId && !isDirty)
            }
            onClick={() => save.mutate()}
            variant="contained"
          >
            {save.isPending ? 'Saving…' : 'Save draft'}
          </Button>
        </Stack>
      </Stack>
      {source.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          Could not load the revision source: {source.error.message}
        </Alert>
      )}
      {save.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          The draft was not saved. {save.error.message}
        </Alert>
      )}
      {!blueprintId && (
        <Dialog
          fullWidth
          maxWidth="md"
          onClose={() => setTemplateDialogOpen(false)}
          open={templateDialogOpen}
        >
          <DialogTitle>Start from an example</DialogTitle>
          <DialogContent>
            <Typography color="text.secondary">
              Choose a valid template, then tailor its TOML in the editor.
            </Typography>
            <Box
              sx={{
                display: 'grid',
                gap: 2,
                gridTemplateColumns: {
                  sm: 'repeat(3, minmax(0, 1fr))',
                  xs: '1fr',
                },
                mt: 2,
              }}
            >
              {blueprintTemplates.map((template, index) => (
                <Card
                  key={template.label}
                  sx={{
                    border: templateIndex === index ? 2 : 1,
                    borderColor:
                      templateIndex === index ? 'primary.main' : 'divider',
                  }}
                  variant="outlined"
                >
                  <CardActionArea onClick={() => selectTemplate(index)}>
                    <CardContent>
                      <Typography component="h3" variant="h6">
                        {template.label}
                      </Typography>
                      <Typography color="text.secondary" sx={{ mt: 1 }}>
                        {template.description}
                      </Typography>
                    </CardContent>
                  </CardActionArea>
                </Card>
              ))}
            </Box>
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setTemplateDialogOpen(false)}>
              Dismiss
            </Button>
          </DialogActions>
        </Dialog>
      )}
      <Box
        sx={{
          border: 1,
          borderColor: 'divider',
          height: 'calc(100vh - 260px)',
          minHeight: 480,
          mt: 3,
        }}
      >
        <Editor
          beforeMount={configureToml}
          defaultLanguage="toml"
          height="100%"
          language="toml"
          onChange={(value) => setEditedDefinition(value ?? '')}
          onMount={(editor, monaco) => {
            editor.addCommand(
              monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS,
              () => {
                if (!save.isPending && (!blueprintId || isDirty)) save.mutate();
              },
            );
          }}
          options={{
            automaticLayout: true,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            tabSize: 2,
            wordWrap: 'on',
          }}
          value={definition}
        />
      </Box>
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {blueprintId && !isDirty
          ? 'Make a change before saving a new draft revision.'
          : 'Press Ctrl+S (or Command+S) to save this draft.'}
      </Typography>
    </PageContainer>
  );
};
