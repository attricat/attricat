import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Box, Button, Paper, Stack, Typography } from '@mui/material';
import { lazy, Suspense, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiRequestError } from '../../api/request';
import { errorMessage } from '../../api/errors';
import { revisionStatuses } from '../../api/revisionStatus';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { LoadingState } from '../../components/LoadingState';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RuleIcon } from '../../components/systemIcons';
import { blueprintKinds } from '../blueprints/constants';
import {
  blueprintCatalogueOptions,
  blueprintRevisionListOptions,
  blueprintRevisionOptions,
} from '../blueprints/queryOptions';
import { contextLabel } from '../contexts/defaultContext';
import { contextListOptions } from '../contexts/queryOptions';
import { definitionKinds } from '../definition-editor/constants';
import {
  DefinitionEditorActions,
  DefinitionSaveHint,
  DefinitionTemplatesButton,
} from '../definition-editor/DefinitionEditorControls';
import { DefinitionWorkspace } from '../definition-editor/DefinitionWorkspace';
import { useDefinitionCheck } from '../definition-editor/useDefinitionCheck';
import { useDefinitionEditor } from '../definition-editor/useDefinitionEditor';
import { draftEditors } from '../drafts/constants';
import { lexiconText } from '../lexicon/lexicon';
import { createRule, updateRule, validateRule, type CreateRule } from './api';
import {
  RULE_DRY_RUN_REQUIRED,
  RULE_HAS_EXISTING_VIOLATIONS,
  ruleRoutes,
} from './constants';
import { ruleQueryKeys } from './queryKeys';
import { ruleOptions } from './queryOptions';
import { RuleChangeDryRun } from './RuleChangeDryRun';
import { RulePreview } from './RulePreview';
import { RuleScopeFields, type RuleScope } from './RuleScopeFields';
import { ruleTemplates } from './templates';

const TomlDiffEditor = lazy(() =>
  import('../../components/TomlDiffEditor').then(({ TomlDiffEditor }) => ({
    default: TomlDiffEditor,
  })),
);
const noScope: RuleScope = { contextId: null };
/** Stands in for a blueprint or context name that is still loading. */
const loadingName = '…';

/** Error codes of a save that an enforcing change's dry run resolves. */
const dryRunGateCodes: readonly string[] = [
  RULE_DRY_RUN_REQUIRED,
  RULE_HAS_EXISTING_VIOLATIONS,
];

/**
 * Writes a new rule for a chosen blueprint, or changes the definition of rule
 * `ruleId`, in the shared definition editor. A rule is checked against the
 * latest published revision of its blueprint.
 */
export const RuleEditorPage = ({ ruleId }: { ruleId?: string }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const creating = ruleId === undefined;
  const rule = useQuery({
    ...ruleOptions(ruleId ?? ''),
    enabled: !creating,
  });
  const source = rule.data;
  const blueprints = useQuery({
    ...blueprintCatalogueOptions(),
    enabled: creating,
  });
  const contexts = useQuery(contextListOptions());
  const [chosenScope, setChosenScope] = useState<RuleScope>(noScope);
  // A rule keeps its blueprint and context.
  const scope: RuleScope = source
    ? { blueprintId: source.blueprint_id, contextId: source.context_id }
    : chosenScope;
  const revisions = useQuery(blueprintRevisionListOptions(scope.blueprintId));
  const latestPublished = revisions.data?.find(
    (revision) => revision.status === revisionStatuses.published,
  );
  const blueprint = useQuery(
    blueprintRevisionOptions(scope.blueprintId, latestPublished?.version),
  );
  const scopeAttributes = useMemo(
    () =>
      blueprint.data?.attributes.map((attribute) => ({
        code: attribute.code,
        targetBlueprint: attribute.target_blueprint_code ?? undefined,
        valueType: attribute.value_type,
      })),
    [blueprint.data],
  );

  const leaveEditor = () =>
    navigate(
      creating
        ? { to: ruleRoutes.list }
        : { params: { ruleId }, to: ruleRoutes.rule },
    );
  const editor = useDefinitionEditor({
    draftEditor: creating ? draftEditors.ruleCreate : draftEditors.ruleEdit,
    draftResource: creating ? [] : [ruleId, source?.version ?? 0],
    onLeave: leaveEditor,
    ready: creating || Boolean(source),
    source: source?.definition,
    templates: ruleTemplates,
  });
  const { definition, isDirty } = editor;
  const { blueprintId, contextId } = scope;
  const scopeReady = blueprintId !== undefined && latestPublished !== undefined;
  const ruleInput = (value: string): CreateRule | undefined =>
    blueprintId
      ? { blueprint_id: blueprintId, context_id: contextId, definition: value }
      : undefined;

  const save = useMutation({
    mutationFn: ({
      value,
      acceptExistingViolations,
    }: {
      value: string;
      acceptExistingViolations?: boolean;
    }) =>
      ruleId === undefined
        ? createRule(ruleInput(value)!)
        : updateRule(ruleId, {
            definition: value,
            accept_existing_violations: acceptExistingViolations,
          }),
    onSuccess: async (saved) => {
      editor.clearDraft();
      await queryClient.invalidateQueries({
        queryKey: ruleQueryKeys.definitions(),
      });
      navigate({ params: { ruleId: saved.id }, to: ruleRoutes.rule });
    },
  });
  // An enforcing change to an enabled rule waits for a dry run of it.
  const dryRunGate =
    save.error instanceof ApiRequestError &&
    dryRunGateCodes.includes(save.error.code ?? '')
      ? save.error
      : undefined;
  const canSave = !save.isPending && scopeReady && (creating || isDirty);
  const saveDefinition = () => {
    if (canSave) save.mutate({ value: definition });
  };
  const check = useDefinitionCheck({
    definition,
    enabled: scopeReady,
    kind: definitionKinds.rule,
    resourceId: ruleId,
    scope: `${blueprintId}:${latestPublished?.version}:${contextId}`,
    validate: (value, _, signal) => validateRule(ruleInput(value)!, signal),
  });

  if (!creating && !source) {
    return (
      <PageContainer>
        {rule.isPending ? (
          <LoadingState>{t('rules.loading')}</LoadingState>
        ) : (
          <QueryErrorNotice
            error={rule.error}
            isRetrying={rule.isFetching}
            onRetry={() => void rule.refetch()}
          />
        )}
      </PageContainer>
    );
  }

  const context = contexts.data?.find(({ id }) => id === contextId);
  const noPublishedRevision =
    blueprintId !== undefined && revisions.isSuccess && !latestPublished;
  return (
    <PageContainer>
      <PageHeader
        actions={
          <DefinitionEditorActions
            canSave={canSave}
            isDirty={isDirty}
            isSaving={save.isPending}
            onCancel={editor.cancel}
            onSave={saveDefinition}
            saveLabel={
              save.isPending ? t('rules.editor.saving') : t('rules.editor.save')
            }
          />
        }
        description={
          source
            ? t('rules.editor.editDescription', { version: source.version })
            : t('rules.editor.newRuleDescription')
        }
        icon={RuleIcon}
        title={
          source
            ? t('rules.editor.editTitle', { name: source.name })
            : t('rules.newRule')
        }
      />
      {creating ? (
        <RuleScopeFields
          blueprints={(blueprints.data ?? []).filter(
            (candidate) => candidate.kind === blueprintKinds.record,
          )}
          contexts={contexts.data ?? []}
          disabled={save.isPending}
          noPublishedRevision={noPublishedRevision}
          onChange={setChosenScope}
          scope={scope}
        />
      ) : (
        <Typography color="text.secondary" sx={{ mt: 2 }} variant="body2">
          {t('rules.editor.fixedScope', {
            blueprint: blueprint.data
              ? lexiconText(blueprint.data.blueprint.name)
              : loadingName,
            context:
              contextId === null
                ? t('rules.recordFindings.allContexts')
                : context
                  ? contextLabel(t, context.code)
                  : loadingName,
          })}
        </Typography>
      )}
      <ApiErrorAlert
        error={
          blueprints.error ??
          revisions.error ??
          blueprint.error ??
          contexts.error
        }
        sx={{ mt: 2 }}
      />
      {dryRunGate ? (
        <Alert
          action={
            dryRunGate.code === RULE_HAS_EXISTING_VIOLATIONS && (
              <Button
                color="inherit"
                disabled={save.isPending}
                onClick={() =>
                  save.mutate({
                    value: definition,
                    acceptExistingViolations: true,
                  })
                }
                size="small"
              >
                {t('rules.editor.saveAnyway')}
              </Button>
            )
          }
          severity="warning"
          sx={{ mt: 3 }}
        >
          {errorMessage(dryRunGate)}
        </Alert>
      ) : (
        <ApiErrorAlert error={save.error} sx={{ mt: 3 }} />
      )}
      {editor.dialogs}
      <DefinitionWorkspace
        check={check}
        kind={definitionKinds.rule}
        onChange={editor.setDefinition}
        onSave={saveDefinition}
        preview={
          check.result && (
            <RulePreview
              attributes={blueprint.data?.attributes}
              rule={check.result}
            />
          )
        }
        readOnly={save.isPending}
        scopeAttributes={scopeAttributes}
        toolbar={
          editor.creating && (
            <DefinitionTemplatesButton
              disabled={save.isPending}
              onClick={editor.openTemplates}
            />
          )
        }
        value={definition}
      />
      {creating && !scopeReady ? (
        <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
          {t('rules.editor.chooseBlueprint')}
        </Typography>
      ) : (
        <DefinitionSaveHint unchanged={!editor.creating && !isDirty} />
      )}
      {source && isDirty && (
        <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
          <Stack
            direction={{ xs: 'column', sm: 'row' }}
            spacing={2}
            sx={{
              alignItems: { sm: 'center' },
              justifyContent: 'space-between',
            }}
          >
            <Box>
              <Typography component="h2" variant="h6">
                {t('rules.editor.changesTitle')}
              </Typography>
              <Typography color="text.secondary" variant="body2">
                {t('rules.editor.changesDescription')}
              </Typography>
            </Box>
            <RuleChangeDryRun
              definition={definition}
              disabled={save.isPending || check.result === undefined}
              blueprintId={source.blueprint_id}
              ruleId={source.id}
            />
          </Stack>
          <Box sx={{ mt: 2 }}>
            <Suspense
              fallback={
                <LoadingState>{t('rules.changelog.loadingDiff')}</LoadingState>
              }
            >
              <TomlDiffEditor
                modified={definition}
                modifiedTitle={t('rules.editor.unsavedChange')}
                original={source.definition}
                originalTitle={t('rules.changelog.currentRevisionOption', {
                  version: source.version,
                })}
              />
            </Suspense>
          </Box>
        </Paper>
      )}
    </PageContainer>
  );
};
