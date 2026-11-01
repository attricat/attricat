import { AuditLogIcon, RecordIcon } from '../../components/systemIcons';
import { EmptyState } from '../../components/EmptyState';
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { ArrowLeftIcon } from 'lucide-react';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Box } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { RouterButton } from '../../components/RouterLink';
import { PageHeader } from '../../components/PageHeader';
import { NextPageButton } from '../../components/LoadMoreButton';
import { RecordSchemaSubheader } from './components/RecordSchemaSubheader';
import { RecordChangeEvent } from './components/RecordChangeEvent';
import { relationshipChangeRecordIds } from './recordChanges';
import { useRecordLabels } from './useRecordLabels';
import { RecordToolbar } from './components/RecordToolbar';
import { getRecordChanges } from './api';
import { recordFormOptions, recordLabelOptions } from './queryOptions';
import type { RecordAuditChange } from './api';
import { recordQueryKeys } from './queryKeys';
import { attributeLabel, displayLabel } from './recordDisplay';
import { BlueprintLink } from '../blueprints/BlueprintLink';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { LoadingState } from '../../components/LoadingState';

type EventChanges = [RecordAuditChange, ...RecordAuditChange[]];

const groupChangesByEvent = (changes: RecordAuditChange[]) => [
  ...changes
    .reduce((groups, change) => {
      const group = groups.get(change.audit_event_id);
      if (group) group.push(change);
      else groups.set(change.audit_event_id, [change]);
      return groups;
    }, new Map<string, EventChanges>())
    .entries(),
];

export const RecordChangesPage = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const changes = useInfiniteQuery({
    queryKey: recordQueryKeys.changes(recordId),
    queryFn: ({ pageParam }) => getRecordChanges(recordId, pageParam),
    initialPageParam: 0,
    getNextPageParam: (page) => page.next_offset,
  });
  const changeItems = changes.data?.pages.flatMap((page) => page.items) ?? [];
  const linkedLabels = useRecordLabels(
    relationshipChangeRecordIds(changeItems),
  );
  const recordForm = useQuery(recordFormOptions(recordId));
  const label = useQuery(recordLabelOptions(recordId));
  const recordDisplay = label.data?.items.find(
    (item) => item.id === recordId,
  )?.display;
  const attributes = useMemo(
    () =>
      new Map(
        (recordForm.data?.blueprint.attributes ?? []).map((attribute) => [
          attribute.code,
          attribute,
        ]),
      ),
    [recordForm.data],
  );
  return (
    <PageContainer maxWidth="lg">
      <PageHeader
        eyebrow={t('records.recordChanges')}
        icon={RecordIcon}
        title={label.isPending ? '' : displayLabel(recordDisplay, recordId)}
      />
      <RecordToolbar label={t('records.recordChanges')}>
        <RouterButton
          color="inherit"
          params={{ recordId }}
          size="small"
          startIcon={<ArrowLeftIcon />}
          to="/records/$recordId"
        >
          {t('records.backToRecord')}
        </RouterButton>
      </RecordToolbar>
      <RecordSchemaSubheader
        name={
          recordForm.data && (
            <BlueprintLink blueprint={recordForm.data.blueprint.blueprint} />
          )
        }
      />
      {changes.isPending && (
        <LoadingState sx={{ py: 3 }}>
          {t('records.loadingChanges')}
        </LoadingState>
      )}
      <ApiErrorAlert error={changes.error} sx={{ mt: 3 }} />
      {changes.data && (
        <Box sx={{ mt: 3 }}>
          {groupChangesByEvent(changeItems).map(([eventId, eventChanges]) => (
            <RecordChangeEvent
              attribute={(code) => attributes.get(code)}
              attributeName={(code) => {
                const attribute = attributes.get(code);
                return attribute ? attributeLabel(attribute) : code;
              }}
              changes={eventChanges}
              key={eventId}
              recordLabels={linkedLabels}
            />
          ))}
          {changeItems.length === 0 && (
            <EmptyState
              icon={AuditLogIcon}
              title={t('records.noRecordedChanges')}
            />
          )}
          {changes.isFetchNextPageError && (
            <ApiErrorAlert error={changes.error} sx={{ mb: 2 }} />
          )}
          <NextPageButton query={changes} />
        </Box>
      )}
    </PageContainer>
  );
};
