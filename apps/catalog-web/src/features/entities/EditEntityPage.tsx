import { useMutation, useQuery } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { BlueprintIcon } from '../../components/system-icons';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import UpgradeOutlinedIcon from '@mui/icons-material/UpgradeOutlined';
import ViewListOutlinedIcon from '@mui/icons-material/ViewListOutlined';
import VisibilityOutlinedIcon from '@mui/icons-material/VisibilityOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/query-keys';
import { defaultContextCode } from '../contexts/constants';
import {
  getEntityForm,
  getCurrentBlueprint,
  getResolvedEntityPreview,
  updateEntity,
} from './api';
import {
  attachReusableAttribute,
  attachReusableAttributeGroup,
  listReusableAttributeGroups,
  listReusableAttributes,
} from '../reusable-attributes/api';
import { reusableAttributeQueryKeys } from '../reusable-attributes/query-keys';
import type { ReusableAttribute } from '../reusable-attributes/schemas';
import { RouterButton, RouterIconButton } from '../../components/RouterLink';
import { EntityContextPicker } from './components/EntityContextPicker';
import { EntityForm } from './components/EntityForm';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityToolbar } from './components/EntityToolbar';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { valuesForForm } from './entity-form';
import { entityQueryKeys } from './query-keys';
import { findEntityHeading } from '../views/components/blocks/EntityHeadingDefinition';
import { resolveHeadingRenderer } from '../views/components/registry';

const editEntityFormId = 'edit-entity-form';

const mostRecentReusableAttributes = (attributes: ReusableAttribute[]) => {
  const latestByDefinition = new Map<string, ReusableAttribute>();
  for (const attribute of attributes) {
    const current = latestByDefinition.get(attribute.definition_id);
    if (!current || attribute.version > current.version) {
      latestByDefinition.set(attribute.definition_id, attribute);
    }
  }
  return [...latestByDefinition.values()].sort(
    (first, second) =>
      first.namespace.localeCompare(second.namespace) ||
      first.code.localeCompare(second.code),
  );
};

export const EditEntityPage = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/entities/$entityId/edit' });
  const [selectedContext, setSelectedContext] = useState('');
  const [isReusableAttributeDialogOpen, setReusableAttributeDialogOpen] =
    useState(false);
  const [reusableSelectionType, setReusableSelectionType] = useState<
    'attribute' | 'group' | null
  >(null);
  const [selectedReusableAttribute, setSelectedReusableAttribute] =
    useState('');
  const [selectedReusableGroup, setSelectedReusableGroup] = useState('');
  const closeReusableAttributeDialog = () => {
    setReusableAttributeDialogOpen(false);
    setReusableSelectionType(null);
    setSelectedReusableAttribute('');
    setSelectedReusableGroup('');
  };
  const entityForm = useQuery({
    queryKey: entityQueryKeys.form(entityId),
    queryFn: () => getEntityForm(entityId),
  });
  const update = useMutation({
    mutationFn: (input: Parameters<typeof updateEntity>[1]) =>
      updateEntity(entityId, input),
    onSuccess: (entity) => {
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      });
    },
  });
  const reusableAttributes = useQuery({
    queryKey: reusableAttributeQueryKeys.definitions(),
    queryFn: ({ signal }) => listReusableAttributes(false, signal),
  });
  const reusableGroups = useQuery({
    queryKey: reusableAttributeQueryKeys.groups(),
    queryFn: ({ signal }) => listReusableAttributeGroups(signal),
  });
  const latestReusableAttributes = mostRecentReusableAttributes(
    reusableAttributes.data ?? [],
  );
  const attach = useMutation({
    mutationFn: (revisionId: string) =>
      attachReusableAttribute(entityId, revisionId),
    onSuccess: () => {
      closeReusableAttributeDialog();
      void entityForm.refetch();
    },
  });
  const attachGroup = useMutation({
    mutationFn: (groupId: string) =>
      attachReusableAttributeGroup(entityId, groupId),
    onSuccess: () => {
      closeReusableAttributeDialog();
      void entityForm.refetch();
    },
  });
  const blueprintId = entityForm.data?.entity.blueprint_id;
  const currentBlueprint = useQuery({
    queryKey: entityQueryKeys.currentBlueprint(blueprintId ?? ''),
    queryFn: () => getCurrentBlueprint(blueprintId!),
    enabled: Boolean(blueprintId),
  });
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const contextId =
    selectedContext ||
    (contexts.data?.find((context) => context.code === defaultContextCode)
      ?.id ??
      null);
  const defaultContextId =
    contexts.data?.find((context) => context.code === defaultContextCode)?.id ??
    null;
  const resolvedPreview = useQuery({
    queryKey: entityQueryKeys.resolvedPreview(entityId, contextId ?? undefined),
    queryFn: () => {
      if (!contextId) throw new Error('Preview context is unavailable');
      return getResolvedEntityPreview(entityId, contextId);
    },
    enabled: contextId !== null,
  });
  const detailView = entityForm.data?.blueprint.blueprint.views?.detail;
  const heading = findEntityHeading(detailView);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  return (
    <PageContainer>
      <PageHeader
        actions={
          entityForm.data && (
            <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
              {entityForm.data.entity.is_sample && (
                <Chip color="info" label={t('entities.sample')} size="small" />
              )}
              <Tooltip title={entityForm.data.blueprint.blueprint.name}>
                <RouterButton
                  params={{ blueprintId: entityForm.data.entity.blueprint_id }}
                  size="small"
                  startIcon={<BlueprintIcon />}
                  to="/manage/blueprints/$blueprintId"
                  variant="text"
                >
                  {t('entities.blueprint')}:{' '}
                  {entityForm.data.blueprint.blueprint.name}
                </RouterButton>
              </Tooltip>
            </Box>
          )
        }
        eyebrow={
          entityForm.data ? (
            <>
              {t('entities.editEntity')} ·{' '}
              <Link
                search={{
                  blueprint: entityForm.data.blueprint.blueprint.code,
                  version: entityForm.data.blueprint.blueprint.version,
                }}
                to="/"
              >
                {t('entities.viewAll')}
              </Link>
            </>
          ) : (
            t('entities.editEntity')
          )
        }
      />
      {entityForm.data && resolvedPreview.data && HeadingRenderer
        ? createElement(HeadingRenderer, {
            attributes: entityForm.data.blueprint.attributes,
            entityId,
            values: resolvedPreview.data.values,
            view: detailView,
          })
        : null}
      <EntityToolbar label={t('entities.editEntity')}>
        <Tooltip title={t('entities.viewPreview')}>
          <RouterIconButton
            aria-label={t('entities.viewPreview')}
            params={{ entityId }}
            to="/entities/$entityId"
          >
            <VisibilityOutlinedIcon />
          </RouterIconButton>
        </Tooltip>
        {entityForm.data && (
          <Tooltip title={t('entities.viewAll')}>
            <RouterIconButton
              aria-label={t('entities.viewAll')}
              search={{
                blueprint: entityForm.data.blueprint.blueprint.code,
                version: entityForm.data.blueprint.blueprint.version,
              }}
              to="/"
            >
              <ViewListOutlinedIcon />
            </RouterIconButton>
          </Tooltip>
        )}
        {currentBlueprint.data &&
          entityForm.data &&
          (currentBlueprint.data.blueprint.version >
          (entityForm.data.entity.blueprint_version ?? Infinity) ? (
            <>
              <Tooltip title={t('entities.schemaOutdated')}>
                <WarningAmberOutlinedIcon color="warning" fontSize="small" />
              </Tooltip>
              <Tooltip title={t('entities.upgradeBlueprint')}>
                <RouterIconButton
                  aria-label={t('entities.upgradeBlueprint')}
                  params={{ entityId }}
                  to="/entities/$entityId/migrate"
                >
                  <UpgradeOutlinedIcon />
                </RouterIconButton>
              </Tooltip>
            </>
          ) : (
            <Tooltip title={t('entities.matchesCurrentSchema')}>
              <CheckCircleOutlinedIcon color="success" fontSize="small" />
            </Tooltip>
          ))}
        {entityForm.data && (
          <Button
            disabled={update.isPending}
            form={editEntityFormId}
            sx={{ ml: 'auto' }}
            type="submit"
            variant="contained"
          >
            {t('entities.saveChanges')}
          </Button>
        )}
      </EntityToolbar>
      <EntitySchemaSubheader
        entityId={entityId}
        name={entityForm.data?.blueprint.blueprint.name}
      />
      {entityForm.isPending && (
        <Typography sx={{ mt: 4 }}>{t('entities.loadingEntity')}</Typography>
      )}
      {(entityForm.error ||
        update.error ||
        attach.error ||
        attachGroup.error ||
        reusableAttributes.error ||
        reusableGroups.error) && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {
            (
              entityForm.error ??
              update.error ??
              attach.error ??
              attachGroup.error ??
              reusableAttributes.error ??
              reusableGroups.error
            )?.message
          }
        </Alert>
      )}
      {resolvedPreview.isError && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {resolvedPreview.error.message}
        </Alert>
      )}
      {entityForm.data && (
        <>
          <Dialog
            fullWidth
            maxWidth="sm"
            onClose={closeReusableAttributeDialog}
            open={isReusableAttributeDialogOpen}
          >
            <DialogTitle>
              {t('entities.addReusableAttributeOrGroup')}
            </DialogTitle>
            <DialogContent>
              {reusableSelectionType === null && (
                <Stack spacing={2} sx={{ pt: 1 }}>
                  <Typography>
                    {t('entities.chooseReusableAttributeOrGroup')}
                  </Typography>
                  <Stack direction={{ sm: 'row' }} spacing={1}>
                    <Button
                      onClick={() => setReusableSelectionType('attribute')}
                      variant="outlined"
                    >
                      {t('entities.additionalAttributes')}
                    </Button>
                    <Button
                      onClick={() => setReusableSelectionType('group')}
                      variant="outlined"
                    >
                      {t('entities.attributeGroup')}
                    </Button>
                  </Stack>
                </Stack>
              )}
              {reusableSelectionType === 'attribute' && (
                <TextField
                  fullWidth
                  label={t('entities.additionalAttributes')}
                  onChange={(event) =>
                    setSelectedReusableAttribute(event.target.value)
                  }
                  select
                  sx={{ mt: 1 }}
                  value={selectedReusableAttribute}
                >
                  <MenuItem value="">
                    {t('entities.selectAdditionalAttribute')}
                  </MenuItem>
                  {latestReusableAttributes.map((attribute) => (
                    <MenuItem key={attribute.id} value={attribute.id}>
                      {attribute.namespace}:{attribute.code} · {attribute.name}{' '}
                      v{attribute.version}
                    </MenuItem>
                  ))}
                </TextField>
              )}
              {reusableSelectionType === 'group' && (
                <TextField
                  fullWidth
                  label={t('entities.attributeGroup')}
                  onChange={(event) =>
                    setSelectedReusableGroup(event.target.value)
                  }
                  select
                  sx={{ mt: 1 }}
                  value={selectedReusableGroup}
                >
                  <MenuItem value="">
                    {t('entities.selectAttributeGroup')}
                  </MenuItem>
                  {(reusableGroups.data ?? []).map((group) => (
                    <MenuItem key={group.id} value={group.id}>
                      {group.name}
                    </MenuItem>
                  ))}
                </TextField>
              )}
            </DialogContent>
            <DialogActions>
              {reusableSelectionType !== null && (
                <Button onClick={() => setReusableSelectionType(null)}>
                  {t('entities.back')}
                </Button>
              )}
              <Button onClick={closeReusableAttributeDialog}>
                {t('common.cancel')}
              </Button>
              {reusableSelectionType === 'attribute' && (
                <Button
                  disabled={
                    !selectedReusableAttribute ||
                    attach.isPending ||
                    reusableAttributes.isPending ||
                    reusableAttributes.isError
                  }
                  onClick={() => attach.mutate(selectedReusableAttribute)}
                  variant="contained"
                >
                  {t('entities.addAttribute')}
                </Button>
              )}
              {reusableSelectionType === 'group' && (
                <Button
                  disabled={
                    !selectedReusableGroup ||
                    attachGroup.isPending ||
                    reusableGroups.isPending ||
                    reusableGroups.isError
                  }
                  onClick={() => attachGroup.mutate(selectedReusableGroup)}
                  variant="contained"
                >
                  {t('entities.addAttributeGroup')}
                </Button>
              )}
            </DialogActions>
          </Dialog>
          <EntityForm
            key={`${entityForm.data.entity.id}:${contextId ?? ''}`}
            blueprint={entityForm.data.blueprint}
            contextId={contextId}
            contextPicker={
              <EntityContextPicker
                contexts={contexts.data ?? []}
                disabled={contexts.isPending}
                onChange={setSelectedContext}
                value={contextId ?? ''}
              />
            }
            entityId={entityId}
            defaultContextId={defaultContextId}
            formId={editEntityFormId}
            footerActions={
              <Button
                onClick={() => setReusableAttributeDialogOpen(true)}
                variant="outlined"
              >
                {t('entities.addReusableAttributeOrGroup')}
              </Button>
            }
            existingValues={[
              ...entityForm.data.values,
              ...entityForm.data.reusable_values,
            ]}
            reusableAttributes={entityForm.data.reusable_attributes}
            resolvedValues={resolvedPreview.data?.values}
            showBlueprintMetadata={false}
            showSubmitButton={false}
            initialValues={valuesForForm(
              [
                ...entityForm.data.blueprint.attributes,
                ...entityForm.data.reusable_attributes,
              ],
              [...entityForm.data.values, ...entityForm.data.reusable_values],
              contextId,
            )}
            isLoadingBlueprint={update.isPending}
            onSubmit={(input) => update.mutate(input)}
            submitLabel={t('entities.saveChanges')}
          />
        </>
      )}
    </PageContainer>
  );
};
