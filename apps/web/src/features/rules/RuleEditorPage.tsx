import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Typography } from '@mui/material';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { revisionStatuses } from '../../api/revisionStatus';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
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
import {
  createRule,
  createRuleRevision,
  validateRule,
  type CreateRule,
} from './api';
import { ruleRoutes } from './constants';
import { ruleQueryKeys } from './queryKeys';
import { ruleDefinitionsOptions } from './queryOptions';
import { RulePreview } from './RulePreview';
import { RuleScopeFields, type RuleScope } from './RuleScopeFields';
import { ruleTemplates } from './templates';

const noScope: RuleScope = { contextId: null };
/** Stands in for a blueprint or context name that is still loading. */
const loadingName = '…';

/**
 * Writes a new rule for a chosen blueprint revision, or a new revision of
 * `ruleId` from its revision `sourceVersion`, in the shared definition editor.
 */
export const RuleEditorPage = ({
  ruleId,
  sourceVersion,
}: {
  ruleId?: string;
  sourceVersion?: number;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const creating = ruleId === undefined;
  const rules = useQuery({ ...ruleDefinitionsOptions(), enabled: !creating });
  const source = rules.data?.find(
    (rule) => rule.id === ruleId && rule.version === sourceVersion,
  );
  const blueprints = useQuery({
    ...blueprintCatalogueOptions(),
    enabled: creating,
  });
  const contexts = useQuery(contextListOptions());
  const [chosenScope, setChosenScope] = useState<RuleScope>(noScope);
  const revisions = useQuery({
    ...blueprintRevisionListOptions(chosenScope.blueprintId),
    enabled: creating,
  });
  const publishedRevisions = revisions.data?.filter(
    (revision) => revision.status === revisionStatuses.published,
  );
  // A revision keeps its rule's scope; a new rule checks the latest published
  // revision of its blueprint unless another is chosen.
  const scope: RuleScope = source
    ? {
        blueprintId: source.blueprint_id,
        blueprintVersion: source.blueprint_version,
        contextId: source.context_id,
      }
    : {
        ...chosenScope,
        blueprintVersion:
          chosenScope.blueprintVersion ?? publishedRevisions?.[0]?.version,
      };
  const blueprint = useQuery(
    blueprintRevisionOptions(scope.blueprintId, scope.blueprintVersion),
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
        : {
            params: { ruleId, version: String(sourceVersion) },
            to: ruleRoutes.revision,
          },
    );
  const editor = useDefinitionEditor({
    draftEditor: creating ? draftEditors.ruleCreate : draftEditors.ruleRevision,
    draftResource: creating ? [] : [ruleId, sourceVersion ?? 0],
    onLeave: leaveEditor,
    ready: creating || Boolean(source),
    source: source?.definition,
    templates: ruleTemplates,
  });
  const { definition, isDirty } = editor;
  const { blueprintId, blueprintVersion, contextId } = scope;
  const ruleInput = (value: string): CreateRule | undefined =>
    blueprintId && blueprintVersion
      ? {
          blueprint_id: blueprintId,
          blueprint_version: blueprintVersion,
          context_id: contextId,
          definition: value,
        }
      : undefined;
  const scopeReady = ruleInput(definition) !== undefined;

  const save = useMutation({
    mutationFn: (input: CreateRule) =>
      creating ? createRule(input) : createRuleRevision(ruleId, input),
    onSuccess: async (rule) => {
      editor.clearDraft();
      await queryClient.invalidateQueries({
        queryKey: ruleQueryKeys.definitions(),
      });
      navigate({
        params: { ruleId: rule.id, version: String(rule.version) },
        to: ruleRoutes.revision,
      });
    },
  });
  const canSave = !save.isPending && scopeReady && (creating || isDirty);
  const saveDefinition = () => {
    const input = ruleInput(definition);
    if (canSave && input) save.mutate(input);
  };
  const check = useDefinitionCheck({
    definition,
    enabled: scopeReady,
    kind: definitionKinds.rule,
    resourceId: ruleId,
    scope: `${blueprintId}:${blueprintVersion}:${contextId}`,
    validate: (value, _, signal) => validateRule(ruleInput(value)!, signal),
  });

  if (!creating && rules.isPending) {
    return (
      <PageContainer>
        <Typography>{t('rules.loading')}</Typography>
      </PageContainer>
    );
  }

  if (!creating && !source) {
    return (
      <PageContainer>
        {rules.isError ? (
          <ApiErrorAlert error={rules.error} />
        ) : (
          <Alert severity="error">{t('rules.revisionPage.notFound')}</Alert>
        )}
      </PageContainer>
    );
  }

  const context = contexts.data?.find(({ id }) => id === contextId);
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
              save.isPending ? t('rules.editor.saving') : t('rules.saveDraft')
            }
          />
        }
        description={
          source
            ? t('rules.editor.newRevisionDescription', {
                version: source.version,
              })
            : t('rules.editor.newRuleDescription')
        }
        icon={RuleIcon}
        title={
          source
            ? t('rules.editor.newRevisionTitle', { name: source.name })
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
          onChange={setChosenScope}
          publishedRevisions={
            chosenScope.blueprintId ? publishedRevisions : undefined
          }
          scope={scope}
        />
      ) : (
        <Typography color="text.secondary" sx={{ mt: 2 }} variant="body2">
          {t('rules.editor.fixedScope', {
            blueprint: t('rules.revisionPage.blueprintRevision', {
              name: blueprint.data
                ? lexiconText(blueprint.data.blueprint.name)
                : loadingName,
              version: blueprintVersion,
            }),
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
      <ApiErrorAlert error={save.error} sx={{ mt: 3 }} />
      {editor.dialogs}
      <DefinitionWorkspace
        check={check}
        kind={definitionKinds.rule}
        onChange={editor.setDefinition}
        onSave={saveDefinition}
        preview={check.result && <RulePreview rule={check.result} />}
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
    </PageContainer>
  );
};
