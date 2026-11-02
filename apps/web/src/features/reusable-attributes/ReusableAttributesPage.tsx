import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Button, Stack, Typography } from '@mui/material';
import { PlusIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  deleteReusableAttributeGroup,
  publishReusableAttributeRevision,
  type ReusableAttributeGroup,
} from './api';
import { latestReusableAttributeRevisions } from './latestRevisions';
import { reusableAttributeQueryKeys } from './queryKeys';
import { ReusableAttributeGroupDialog } from './ReusableAttributeGroupDialog';
import { ReusableAttributeGroupsTable } from './ReusableAttributeGroupsTable';
import { ReusableAttributesTable } from './ReusableAttributesTable';
import { ReusableAttributeIcon } from '../../components/systemIcons';
import { useCan } from '../auth/useCan';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import {
  reusableAttributeListOptions,
  reusableAttributeGroupListOptions,
} from './queryOptions';
import { LoadingState } from '../../components/LoadingState';

export const ReusableAttributesPage = () => {
  const { t } = useTranslation();
  const canEdit = useCan(capabilities.blueprintsWrite);
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [editingGroup, setEditingGroup] = useState<{
    group?: ReusableAttributeGroup;
  } | null>(null);
  const [deletingGroup, setDeletingGroup] = useState<{
    group: ReusableAttributeGroup;
    open: boolean;
  } | null>(null);
  const attributes = useQuery(reusableAttributeListOptions(true));
  const groups = useQuery(reusableAttributeGroupListOptions());
  const publish = useMutation({
    mutationFn: publishReusableAttributeRevision,
    onSuccess: () =>
      void queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.root(),
      }),
  });
  const hideDeleteGroup = () =>
    setDeletingGroup((current) => current && { ...current, open: false });
  const removeGroup = useMutation({
    mutationFn: (group: ReusableAttributeGroup) =>
      deleteReusableAttributeGroup(group.id),
    onSuccess: () => {
      // Close before refetching: a slow refetch must not hold the dialog.
      hideDeleteGroup();
      void queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.groups(),
      });
    },
  });
  const displayedAttributes = latestReusableAttributeRevisions(
    attributes.data ?? [],
  );

  return (
    <PageContainer>
      <PageHeader
        description={t('reusableAttributes.description')}
        icon={ReusableAttributeIcon}
        title={t('reusableAttributes.title')}
        actions={
          canEdit && (
            <Stack direction="row" spacing={1}>
              <Button
                onClick={() => setEditingGroup({})}
                startIcon={<PlusIcon />}
                variant="outlined"
              >
                {t('reusableAttributes.newGroup')}
              </Button>
              <Button
                onClick={() =>
                  navigate({ to: '/manage/reusable-attributes/new' })
                }
                startIcon={<PlusIcon />}
                variant="contained"
              >
                {t('reusableAttributes.newAttribute')}
              </Button>
            </Stack>
          )
        }
      />
      <ApiErrorAlert
        error={attributes.error ?? groups.error ?? publish.error}
        sx={{ mt: 3 }}
      />
      {attributes.isPending ? (
        <LoadingState sx={{ mt: 3 }}>
          {t('reusableAttributes.loading')}
        </LoadingState>
      ) : (
        <ReusableAttributesTable
          attributes={displayedAttributes}
          canEdit={canEdit}
          onPublish={(revisionId) => publish.mutate(revisionId)}
          publishing={publish.isPending}
        />
      )}
      <Typography component="h2" sx={{ mt: 4 }} variant="h5">
        {t('reusableAttributes.groupsHeading')}
      </Typography>
      {groups.isPending ? (
        <LoadingState sx={{ mt: 2 }}>
          {t('reusableAttributes.loadingGroups')}
        </LoadingState>
      ) : (
        <ReusableAttributeGroupsTable
          canEdit={canEdit}
          groups={groups.data ?? []}
          onDelete={(group) => {
            removeGroup.reset();
            setDeletingGroup({ group, open: true });
          }}
          onEdit={(group) => setEditingGroup({ group })}
        />
      )}
      {editingGroup && (
        <ReusableAttributeGroupDialog
          attributes={attributes.data ?? []}
          group={editingGroup.group}
          onClose={() => setEditingGroup(null)}
        />
      )}
      <ConfirmDialog
        confirmLabel={t('reusableAttributes.deleteGroup')}
        destructive
        error={removeGroup.error}
        isPending={removeGroup.isPending}
        onClose={hideDeleteGroup}
        onConfirm={() =>
          deletingGroup?.open && removeGroup.mutate(deletingGroup.group)
        }
        open={Boolean(deletingGroup?.open)}
        slotProps={{ transition: { onExited: () => setDeletingGroup(null) } }}
        title={t('reusableAttributes.deleteGroup')}
      >
        {t('reusableAttributes.deleteGroupConfirm', {
          group: deletingGroup?.group.name,
        })}
      </ConfirmDialog>
    </PageContainer>
  );
};
