import { Editor } from '@monaco-editor/react';
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
  DialogContentText,
  DialogTitle,
  Stack,
  Typography,
} from '@mui/material';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import {
  createBlueprint,
  createBlueprintRevision,
  getBlueprintRevision,
} from './api';
import { blueprintQueryKeys } from './query-keys';
import { blueprintTemplates, configureToml } from './blueprint-editor-utils';

type PendingUnsavedAction =
  { templateIndex: number; type: 'replace' } | { type: 'discard' };

export const BlueprintEditorPage = ({
  blueprintId,
  sourceVersion,
}: {
  blueprintId?: string;
  sourceVersion?: number;
}) => {
  const { t } = useTranslation();
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
  const [pendingUnsavedAction, setPendingUnsavedAction] =
    useState<PendingUnsavedAction>();
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

  const applyTemplate = (index: number) => {
    setTemplateIndex(index);
    setEditedDefinition(null);
    setTemplateDialogOpen(false);
  };

  const selectTemplate = (index: number) => {
    if (isDirty) {
      setPendingUnsavedAction({ templateIndex: index, type: 'replace' });
      return;
    }
    applyTemplate(index);
  };

  const leaveEditor = () => {
    navigate(
      blueprintId
        ? { params: { blueprintId }, to: '/manage/blueprints/$blueprintId' }
        : { to: '/manage/blueprints' },
    );
  };

  const cancel = () => {
    if (isDirty) {
      setPendingUnsavedAction({ type: 'discard' });
      return;
    }
    leaveEditor();
  };

  const confirmUnsavedAction = () => {
    if (pendingUnsavedAction?.type === 'replace') {
      applyTemplate(pendingUnsavedAction.templateIndex);
    } else if (pendingUnsavedAction?.type === 'discard') {
      leaveEditor();
    }
    setPendingUnsavedAction(undefined);
  };

  if (blueprintId && source.isPending) {
    return (
      <PageContainer>
        <Typography>{t('blueprints.loadingBlueprintSource')}</Typography>
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
              ? t('blueprints.newRevisionDescription', {
                  version: sourceVersion,
                })
              : t('blueprints.newBlueprintDescription')
          }
          title={
            blueprintId
              ? t('blueprints.newBlueprintRevision')
              : t('blueprints.newBlueprint')
          }
        />
        <Stack direction="row" spacing={1}>
          {!blueprintId && (
            <Button onClick={() => setTemplateDialogOpen(true)}>
              {t('blueprints.examples')}
            </Button>
          )}
          {isDirty && (
            <Chip color="warning" label={t('blueprints.unsavedChanges')} />
          )}
          <Button disabled={save.isPending} onClick={cancel}>
            {t('blueprints.cancel')}
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
            {save.isPending
              ? t('blueprints.saving')
              : t('blueprints.saveDraft')}
          </Button>
        </Stack>
      </Stack>
      {source.data?.attributes.map((attribute) => (
        <ExtensionOutlet
          context={{
            attribute_id: attribute.id,
            blueprint_id: source.data!.blueprint.id,
            blueprint_version: source.data!.blueprint.version,
          }}
          key={attribute.id}
          outlet="blueprint_attribute_configuration"
        />
      ))}
      {source.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('blueprints.couldNotLoadSource', {
            message: source.error.message,
          })}
        </Alert>
      )}
      {save.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('blueprints.draftNotSaved', { message: save.error.message })}
        </Alert>
      )}
      {!blueprintId && (
        <Dialog
          fullWidth
          maxWidth="md"
          onClose={() => setTemplateDialogOpen(false)}
          open={templateDialogOpen}
        >
          <DialogTitle>{t('blueprints.startFromExample')}</DialogTitle>
          <DialogContent>
            <Typography color="text.secondary">
              {t('blueprints.templateDescription')}
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
                  key={template.labelKey}
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
                        {t(template.labelKey)}
                      </Typography>
                      <Typography color="text.secondary" sx={{ mt: 1 }}>
                        {t(template.descriptionKey)}
                      </Typography>
                    </CardContent>
                  </CardActionArea>
                </Card>
              ))}
            </Box>
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setTemplateDialogOpen(false)}>
              {t('blueprints.dismiss')}
            </Button>
          </DialogActions>
        </Dialog>
      )}
      <Dialog
        aria-describedby="unsaved-changes-dialog-description"
        onClose={() => setPendingUnsavedAction(undefined)}
        open={Boolean(pendingUnsavedAction)}
      >
        <DialogTitle>{t('blueprints.unsavedChangesTitle')}</DialogTitle>
        <DialogContent>
          <DialogContentText id="unsaved-changes-dialog-description">
            {pendingUnsavedAction?.type === 'replace'
              ? t('blueprints.replaceUnsavedChanges')
              : t('blueprints.discardUnsavedChanges')}
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setPendingUnsavedAction(undefined)}>
            {t('blueprints.keepEditing')}
          </Button>
          <Button
            color="error"
            onClick={confirmUnsavedAction}
            variant="contained"
          >
            {pendingUnsavedAction?.type === 'replace'
              ? t('blueprints.replace')
              : t('blueprints.discard')}
          </Button>
        </DialogActions>
      </Dialog>
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
          ? t('blueprints.makeChangeBeforeSaving')
          : t('blueprints.saveShortcut')}
      </Typography>
    </PageContainer>
  );
};
