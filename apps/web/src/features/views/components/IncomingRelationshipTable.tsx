import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogContent,
  DialogTitle,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { PanelRightOpenIcon, PlusIcon, Trash2Icon } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../../components/ApiErrorAlert';
import { LabeledIconButton } from '../../../components/LabeledIconButton';
import { compactIconSize } from '../../../components/iconSizes';
import { useToast } from '../../../components/useToast';
import { visuallyHiddenSx } from '../../../components/VisuallyHidden';
import { findDefaultContextId } from '../../contexts/defaultContext';
import { contextListOptions } from '../../contexts/queryOptions';
import { ExplorerRecordPanel } from '../../explorer/ExplorerRecordPanel';
import { lexiconText } from '../../lexicon/lexicon';
import { createRecord, searchRecords } from '../../records/api';
import { DeleteRecordDialog } from '../../records/components/DeleteRecordDialog';
import { RecordForm } from '../../records/components/RecordForm';
import { invalidateRecordSearches } from '../../records/invalidateRecord';
import {
  blueprintByCodeOptions,
  recordFormOptions,
} from '../../records/queryOptions';
import { recordQueryKeys } from '../../records/queryKeys';
import { attributeLabel } from '../../records/recordDisplay';
import { attributeValueKinds } from '../../records/valueTypes';
import type { ViewNode } from '../schemas';
import { IncomingEditableRow } from './IncomingEditableRow';
import { incomingTableColumns } from './incomingTableColumns';

type Node = Extract<ViewNode, { type: 'incoming_relationship_list' }>;
type Relationship = Node['relationships'][number];

/** Opens a linked record in the side panel, or deletes it. */
const RowActions = ({
  editable,
  label,
  onDelete,
  onOpen,
}: {
  editable: boolean;
  label: string;
  onDelete: () => void;
  onOpen: () => void;
}) => {
  const { t } = useTranslation();
  return (
    <>
      <LabeledIconButton
        label={t('views.incomingTable.open', { label })}
        onClick={onOpen}
        size="small"
      >
        <PanelRightOpenIcon size={compactIconSize} />
      </LabeledIconButton>
      {editable && (
        <LabeledIconButton
          label={t('views.incomingTable.delete', { label })}
          onClick={onDelete}
          size="small"
        >
          <Trash2Icon size={compactIconSize} />
        </LabeledIconButton>
      )}
    </>
  );
};

/**
 * Creates a record of the source blueprint linked to the page's record. The
 * link is set and cannot be changed in the form.
 */
const AddIncomingRecordDialog = ({
  blueprintCode,
  field,
  name,
  onClose,
  recordId,
}: {
  blueprintCode: string;
  field: string;
  name: string;
  onClose: () => void;
  recordId: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const { show } = useToast();
  const [formKey, setFormKey] = useState(0);
  const blueprint = useQuery(blueprintByCodeOptions(blueprintCode));
  const contexts = useQuery(contextListOptions());
  const defaultContextId = findDefaultContextId(contexts.data) ?? undefined;
  const initialValues = useMemo(
    () => ({ [field]: recordId }),
    [field, recordId],
  );
  // The link to this record is fixed; only the other fields are entered.
  const formBlueprint = useMemo(
    () =>
      blueprint.data && {
        ...blueprint.data,
        attributes: blueprint.data.attributes.map((attribute) =>
          attribute.code === field
            ? { ...attribute, readonly: true }
            : attribute,
        ),
      },
    [blueprint.data, field],
  );
  const create = useMutation({
    meta: { toast: false },
    mutationFn: async ({
      values,
      relationships,
    }: {
      values: Parameters<typeof createRecord>[0]['values'];
      relationships: { attribute_code: string; target_record_ids: string[] }[];
      addAnother: boolean;
    }) => {
      if (!blueprint.data || !defaultContextId)
        throw new Error(t('records.defaultContextUnavailable'));
      // The fixed link is always sent, whatever the form returns for it.
      const links = [
        ...relationships.filter(
          (relationship) => relationship.attribute_code !== field,
        ),
        { attribute_code: field, target_record_ids: [recordId] },
      ];
      return createRecord({
        blueprint: {
          code: blueprint.data.blueprint.code,
          version: blueprint.data.blueprint.version,
        },
        values: [
          ...values
            .filter((value) => value.attribute_code !== field)
            .map((value) => ({ ...value, context_id: defaultContextId })),
          ...links.flatMap((relationship) =>
            relationship.target_record_ids.map((target_record_id) => ({
              kind: attributeValueKinds.relationship,
              attribute_code: relationship.attribute_code,
              context_id: defaultContextId,
              target_record_id,
            })),
          ),
        ],
      });
    },
    onSuccess: (_record, { addAnother }) => {
      void invalidateRecordSearches(client);
      show({
        message: t('views.incomingTable.added', { name }),
        severity: 'success',
      });
      if (addAnother) setFormKey((key) => key + 1);
      else onClose();
    },
  });
  return (
    <Dialog fullWidth maxWidth="md" onClose={onClose} open>
      <DialogTitle>{t('views.incomingTable.addTitle', { name })}</DialogTitle>
      <DialogContent>
        <RecordForm
          addAnotherLabel={t('views.incomingTable.addAnother')}
          blueprint={formBlueprint}
          contextId={defaultContextId}
          defaultContextId={defaultContextId}
          error={blueprint.error ?? create.error}
          initialValues={initialValues}
          isLoadingBlueprint={blueprint.isLoading}
          isSubmitting={create.isPending}
          key={formKey}
          lockedBlueprint
          onCancel={onClose}
          onSubmit={({ values, relationships, addAnother }) =>
            create.mutate({ values, relationships, addAnother })
          }
          submitLabel={t('views.incomingTable.addSubmit')}
        />
      </DialogContent>
    </Dialog>
  );
};

/**
 * The records linking to this one through one relationship, as a table on
 * the record's page, such as an order's lines. An editable table edits their
 * values in its cells, and adds and deletes them, until the record's status
 * settles them.
 */
export const IncomingRelationshipTable = ({
  recordId,
  node,
}: {
  recordId: string;
  node: Node;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const relationship: Relationship = node.relationships[0];
  const [adding, setAdding] = useState(false);
  const [opened, setOpened] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const contexts = useQuery(contextListOptions());
  const defaultContextId = findDefaultContextId(contexts.data);
  const owner = useQuery(recordFormOptions(recordId));
  const rows = useQuery({
    // Under the record searches, so record changes refresh it.
    queryKey: [
      ...recordQueryKeys.searches(),
      'incoming-table',
      recordId,
      relationship.source_blueprint,
      relationship.field,
      node.page_size,
    ],
    queryFn: ({ signal }) =>
      searchRecords({
        blueprint: relationship.source_blueprint,
        includeTotal: true,
        pageSize: node.page_size,
        relationshipFilters: [
          { field: relationship.field, selected_target_ids: [recordId] },
        ],
        signal,
      }),
  });
  const blueprint = rows.data?.blueprint;
  const columns = blueprint
    ? incomingTableColumns(blueprint, node, relationship.field)
    : [];
  const name = blueprint ? lexiconText(blueprint.blueprint.name) : '';
  const settled = (owner.data?.settled_lines ?? []).some(
    (lines) =>
      lines.blueprint === relationship.source_blueprint &&
      lines.relationship === relationship.field,
  );
  const editable = Boolean(node.editable && owner.data?.can_write && !settled);
  const items = rows.data?.items ?? [];
  const total = rows.data?.total_count ?? items.length;
  const label = lexiconText(node.label);

  return (
    <Box component="section" aria-label={label}>
      <Stack
        direction="row"
        sx={{ alignItems: 'center', justifyContent: 'space-between', mb: 1 }}
      >
        <Typography component="h3" variant="subtitle1">
          {label}
        </Typography>
        {editable && blueprint && (
          <Button
            onClick={() => setAdding(true)}
            size="small"
            startIcon={<PlusIcon size={compactIconSize} />}
            variant="outlined"
          >
            {t('views.incomingTable.add', { name })}
          </Button>
        )}
      </Stack>
      {node.editable && settled && (
        <Alert severity="info" sx={{ mb: 1 }}>
          {t('views.incomingTable.settled')}
        </Alert>
      )}
      {rows.error && <ApiErrorAlert error={rows.error} />}
      {blueprint && (
        <TableContainer
          sx={{ border: 1, borderColor: 'divider', borderRadius: 1 }}
        >
          <Table aria-label={label} size="small">
            <TableHead>
              <TableRow>
                {columns.map((attribute) => (
                  <TableCell key={attribute.code}>
                    {attributeLabel(attribute)}
                  </TableCell>
                ))}
                <TableCell align="right">
                  <Box component="span" sx={visuallyHiddenSx}>
                    {t('views.incomingTable.actions')}
                  </Box>
                </TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {items.map((item) => (
                <IncomingEditableRow
                  actions={
                    <RowActions
                      editable={editable}
                      // A record without a label is named by its blueprint.
                      label={
                        item.display.default ||
                        Object.values(item.display).find(Boolean) ||
                        name
                      }
                      onDelete={() => setDeleting(item.id)}
                      onOpen={() => setOpened(item.id)}
                    />
                  }
                  columns={columns}
                  contextId={defaultContextId}
                  editable={editable}
                  key={item.id}
                  rowId={item.id}
                />
              ))}
              {items.length === 0 && (
                <TableRow>
                  <TableCell colSpan={columns.length + 1}>
                    <Typography color="text.secondary" variant="body2">
                      {t('views.incomingTable.empty')}
                    </Typography>
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </TableContainer>
      )}
      {total > items.length && (
        <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
          {t('views.incomingTable.more', { shown: items.length, total })}
        </Typography>
      )}
      {adding && blueprint && (
        <AddIncomingRecordDialog
          blueprintCode={relationship.source_blueprint}
          field={relationship.field}
          name={name}
          onClose={() => setAdding(false)}
          recordId={recordId}
        />
      )}
      {opened && (
        <ExplorerRecordPanel
          onClose={() => {
            setOpened(null);
            void invalidateRecordSearches(client);
          }}
          onOpenRecord={setOpened}
          recordId={opened}
        />
      )}
      {deleting && (
        <DeleteRecordDialog
          onClose={() => setDeleting(null)}
          onDeleted={() => {
            setDeleting(null);
            return invalidateRecordSearches(client);
          }}
          recordId={deleting}
        />
      )}
    </Box>
  );
};
