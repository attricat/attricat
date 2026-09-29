import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import AddIcon from '@mui/icons-material/Add';
import { Alert, Button, Stack, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  listReusableAttributeGroups,
  listReusableAttributes,
  publishReusableAttributeRevision,
} from './api';
import { latestReusableAttributeRevisions } from './latestRevisions';
import { reusableAttributeQueryKeys } from './queryKeys';
import { ReusableAttributeGroupDialog } from './ReusableAttributeGroupDialog';
import { ReusableAttributeGroupsTable } from './ReusableAttributeGroupsTable';
import { ReusableAttributesTable } from './ReusableAttributesTable';

export const ReusableAttributesPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [groupOpen, setGroupOpen] = useState(false);
  const attributes = useQuery({
    queryKey: reusableAttributeQueryKeys.definitions(true),
    queryFn: ({ signal }) => listReusableAttributes(true, signal),
  });
  const groups = useQuery({
    queryKey: reusableAttributeQueryKeys.groups(),
    queryFn: ({ signal }) => listReusableAttributeGroups(signal),
  });
  const publish = useMutation({
    mutationFn: publishReusableAttributeRevision,
    onSuccess: () =>
      void queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.root(),
      }),
  });
  const displayedAttributes = latestReusableAttributeRevisions(
    attributes.data ?? [],
  );

  return (
    <PageContainer>
      <PageHeader
        description={t('reusableAttributes.description')}
        title={t('reusableAttributes.title')}
        actions={
          <Stack direction="row" spacing={1}>
            <Button onClick={() => setGroupOpen(true)} variant="outlined">
              {t('reusableAttributes.newGroup')}
            </Button>
            <Button
              onClick={() =>
                navigate({ to: '/manage/reusable-attributes/new' })
              }
              startIcon={<AddIcon />}
              variant="contained"
            >
              {t('reusableAttributes.newAttribute')}
            </Button>
          </Stack>
        }
      />
      {(attributes.error || groups.error || publish.error) && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {attributes.error?.message ??
            groups.error?.message ??
            publish.error?.message}
        </Alert>
      )}
      {attributes.isPending ? (
        <Typography sx={{ mt: 3 }}>
          {t('reusableAttributes.loading')}
        </Typography>
      ) : (
        <ReusableAttributesTable
          attributes={displayedAttributes}
          onPublish={(revisionId) => publish.mutate(revisionId)}
          publishing={publish.isPending}
        />
      )}
      <Typography sx={{ mt: 4 }} variant="h5">
        {t('reusableAttributes.groupsHeading')}
      </Typography>
      {groups.isPending ? (
        <Typography sx={{ mt: 2 }}>
          {t('reusableAttributes.loadingGroups')}
        </Typography>
      ) : (
        <ReusableAttributeGroupsTable groups={groups.data ?? []} />
      )}
      {groupOpen && (
        <ReusableAttributeGroupDialog
          attributes={attributes.data ?? []}
          onClose={() => setGroupOpen(false)}
        />
      )}
    </PageContainer>
  );
};
