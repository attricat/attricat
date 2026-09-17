import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import AddIcon from '@mui/icons-material/Add';
import EditIcon from '@mui/icons-material/Edit';
import PublishIcon from '@mui/icons-material/Publish';
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  MenuItem,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  createReusableAttribute,
  createReusableAttributeGroup,
  createReusableAttributeRevision,
  listReusableAttributeGroups,
  listReusableAttributes,
  publishReusableAttributeRevision,
  type ReusableAttribute,
} from './api';
import { reusableAttributeQueryKeys } from './query-keys';
import { reusableAttributeValueTypeSchema } from './schemas';

type AttributeFormValues = {
  namespace: string;
  code: string;
  name: string;
  value_type: (typeof reusableAttributeValueTypeSchema.options)[number];
  target_blueprint_code: string;
  cardinality: 'one' | 'many';
  context_fallback: 'default' | 'none';
  context_editable: 'all' | 'default';
  tags: string;
  readonly: boolean;
  searchable: boolean;
  facetable: boolean;
};

const defaultAttribute: AttributeFormValues = {
  namespace: '',
  code: '',
  name: '',
  value_type: 'string',
  target_blueprint_code: '',
  cardinality: 'one',
  context_fallback: 'default',
  context_editable: 'all',
  tags: '',
  readonly: false,
  searchable: false,
  facetable: false,
};

const toRequest = (value: AttributeFormValues) => ({
  namespace: value.namespace.trim(),
  code: value.code.trim(),
  name: value.name.trim(),
  value_type: value.value_type,
  target_blueprint_code:
    value.value_type === 'relationship' && value.target_blueprint_code.trim()
      ? value.target_blueprint_code.trim()
      : null,
  cardinality: value.value_type === 'relationship' ? value.cardinality : null,
  target_cardinality: null,
  tags: value.tags
    .split(',')
    .map((tag) => tag.trim())
    .filter(Boolean),
  context_fallback: value.context_fallback,
  context_editable: value.context_editable,
  readonly: value.readonly,
  searchable: value.searchable,
  facetable: value.facetable,
});

const AttributeDialog = ({
  attribute,
  onClose,
}: {
  attribute?: ReusableAttribute;
  onClose: () => void;
}) => {
  const queryClient = useQueryClient();
  const [validationError, setValidationError] = useState<string>();
  const save = useMutation({
    mutationFn: (value: AttributeFormValues) =>
      attribute
        ? createReusableAttributeRevision(
            attribute.definition_id,
            toRequest(value),
          )
        : createReusableAttribute(toRequest(value)),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.root(),
      });
      onClose();
    },
  });
  const form = useForm({
    defaultValues: attribute
      ? {
          namespace: attribute.namespace,
          code: attribute.code,
          name: attribute.name,
          value_type: attribute.value_type,
          target_blueprint_code: attribute.target_blueprint_code ?? '',
          cardinality: attribute.cardinality ?? 'one',
          context_fallback: attribute.context_fallback,
          context_editable: attribute.context_editable,
          tags: attribute.tags.join(', '),
          readonly: attribute.readonly,
          searchable: attribute.searchable,
          facetable: attribute.facetable,
        }
      : defaultAttribute,
    onSubmit: ({ value }) => {
      setValidationError(undefined);
      if (!value.namespace.trim() || !value.code.trim() || !value.name.trim()) {
        setValidationError('Namespace, code, and name are required.');
        return;
      }
      if (
        value.value_type === 'relationship' &&
        !value.target_blueprint_code.trim()
      ) {
        setValidationError(
          'Relationship attributes require a target blueprint code.',
        );
        return;
      }
      save.mutate(value);
    },
  });
  return (
    <Dialog fullWidth maxWidth="sm" onClose={onClose} open>
      <DialogTitle>
        {attribute
          ? `New revision for ${attribute.name}`
          : 'New reusable attribute'}
      </DialogTitle>
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2}>
              <form.Field name="namespace">
                {(field) => (
                  <TextField
                    disabled={Boolean(attribute)}
                    fullWidth
                    helperText={
                      attribute
                        ? 'Namespace is fixed for all revisions.'
                        : undefined
                    }
                    label="Namespace"
                    onChange={(event) => field.handleChange(event.target.value)}
                    required
                    value={field.state.value}
                  />
                )}
              </form.Field>
              <form.Field name="code">
                {(field) => (
                  <TextField
                    disabled={Boolean(attribute)}
                    fullWidth
                    helperText={
                      attribute
                        ? 'Code is fixed for all revisions.'
                        : undefined
                    }
                    label="Code"
                    onChange={(event) => field.handleChange(event.target.value)}
                    required
                    value={field.state.value}
                  />
                )}
              </form.Field>
            </Stack>
            <form.Field name="name">
              {(field) => (
                <TextField
                  fullWidth
                  label="Name"
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="value_type">
              {(field) => (
                <TextField
                  fullWidth
                  label="Value type"
                  onChange={(event) =>
                    field.handleChange(
                      event.target.value as AttributeFormValues['value_type'],
                    )
                  }
                  select
                  value={field.state.value}
                >
                  {reusableAttributeValueTypeSchema.options.map((type) => (
                    <MenuItem key={type} value={type}>
                      {type}
                    </MenuItem>
                  ))}
                </TextField>
              )}
            </form.Field>
            <form.Subscribe selector={(state) => state.values.value_type}>
              {(valueType) =>
                valueType === 'relationship' && (
                  <>
                    <form.Field name="target_blueprint_code">
                      {(field) => (
                        <TextField
                          fullWidth
                          label="Target blueprint code"
                          onChange={(event) =>
                            field.handleChange(event.target.value)
                          }
                          required
                          value={field.state.value}
                        />
                      )}
                    </form.Field>
                    <form.Field name="cardinality">
                      {(field) => (
                        <TextField
                          fullWidth
                          label="Cardinality"
                          onChange={(event) =>
                            field.handleChange(
                              event.target.value as 'one' | 'many',
                            )
                          }
                          select
                          value={field.state.value}
                        >
                          <MenuItem value="one">One</MenuItem>
                          <MenuItem value="many">Many</MenuItem>
                        </TextField>
                      )}
                    </form.Field>
                  </>
                )
              }
            </form.Subscribe>
            <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2}>
              <form.Field name="context_fallback">
                {(field) => (
                  <TextField
                    fullWidth
                    label="Context fallback"
                    onChange={(event) =>
                      field.handleChange(
                        event.target.value as 'default' | 'none',
                      )
                    }
                    select
                    value={field.state.value}
                  >
                    <MenuItem value="default">Default context</MenuItem>
                    <MenuItem value="none">No fallback</MenuItem>
                  </TextField>
                )}
              </form.Field>
              <form.Field name="context_editable">
                {(field) => (
                  <TextField
                    fullWidth
                    label="Context editing"
                    onChange={(event) =>
                      field.handleChange(
                        event.target.value as 'all' | 'default',
                      )
                    }
                    select
                    value={field.state.value}
                  >
                    <MenuItem value="all">All contexts</MenuItem>
                    <MenuItem value="default">Default context only</MenuItem>
                  </TextField>
                )}
              </form.Field>
            </Stack>
            <form.Field name="tags">
              {(field) => (
                <TextField
                  fullWidth
                  helperText="Comma-separated"
                  label="Tags"
                  onChange={(event) => field.handleChange(event.target.value)}
                  value={field.state.value}
                />
              )}
            </form.Field>
            <Stack direction={{ xs: 'column', sm: 'row' }}>
              <form.Field name="readonly">
                {(field) => (
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={field.state.value}
                        onChange={(event) =>
                          field.handleChange(event.target.checked)
                        }
                      />
                    }
                    label="Read only"
                  />
                )}
              </form.Field>
              <form.Field name="searchable">
                {(field) => (
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={field.state.value}
                        onChange={(event) =>
                          field.handleChange(event.target.checked)
                        }
                      />
                    }
                    label="Searchable"
                  />
                )}
              </form.Field>
              <form.Field name="facetable">
                {(field) => (
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={field.state.value}
                        onChange={(event) =>
                          field.handleChange(event.target.checked)
                        }
                      />
                    }
                    label="Facetable"
                  />
                )}
              </form.Field>
            </Stack>
            {(validationError || save.error) && (
              <Alert severity="error">
                {validationError ?? save.error?.message}
              </Alert>
            )}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose}>Cancel</Button>
          <Button disabled={save.isPending} type="submit" variant="contained">
            {attribute ? 'Create revision' : 'Create attribute'}
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
};

const GroupDialog = ({
  attributes,
  onClose,
}: {
  attributes: ReusableAttribute[];
  onClose: () => void;
}) => {
  const queryClient = useQueryClient();
  const [validationError, setValidationError] = useState<string>();
  const save = useMutation({
    mutationFn: createReusableAttributeGroup,
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.groups(),
      });
      onClose();
    },
  });
  const form = useForm({
    defaultValues: {
      code: '',
      name: '',
      position: '0',
      revisionIds: [] as string[],
    },
    onSubmit: ({ value }) => {
      setValidationError(undefined);
      if (
        !value.code.trim() ||
        !value.name.trim() ||
        value.revisionIds.length === 0
      ) {
        setValidationError(
          'Code, name, and at least one published revision are required.',
        );
        return;
      }
      const position = Number(value.position);
      if (!Number.isInteger(position) || position < 0) {
        setValidationError('Position must be a non-negative whole number.');
        return;
      }
      save.mutate({
        code: value.code.trim(),
        name: value.name.trim(),
        position,
        reusable_attribute_revision_ids: value.revisionIds,
      });
    },
  });
  const published = attributes.filter(
    (attribute) => attribute.status === 'published',
  );
  return (
    <Dialog fullWidth maxWidth="sm" onClose={onClose} open>
      <DialogTitle>New reusable attribute group</DialogTitle>
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            <form.Field name="code">
              {(field) => (
                <TextField
                  label="Code"
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="name">
              {(field) => (
                <TextField
                  label="Name"
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="position">
              {(field) => (
                <TextField
                  slotProps={{ htmlInput: { min: 0, step: 1 } }}
                  label="Position"
                  onChange={(event) => field.handleChange(event.target.value)}
                  type="number"
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="revisionIds">
              {(field) => (
                <TextField
                  helperText="Only published revisions can be grouped."
                  label="Published revisions"
                  onChange={(event) =>
                    field.handleChange(
                      event.target.value as unknown as string[],
                    )
                  }
                  slotProps={{ select: { multiple: true } }}
                  select
                  value={field.state.value}
                >
                  {published.map((attribute) => (
                    <MenuItem key={attribute.id} value={attribute.id}>
                      {attribute.namespace}:{attribute.code} · v
                      {attribute.version}
                    </MenuItem>
                  ))}
                </TextField>
              )}
            </form.Field>
            {(validationError || save.error) && (
              <Alert severity="error">
                {validationError ?? save.error?.message}
              </Alert>
            )}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            disabled={save.isPending || published.length === 0}
            type="submit"
            variant="contained"
          >
            Create group
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
};

export const ReusableAttributesPage = () => {
  const queryClient = useQueryClient();
  const [dialogAttribute, setDialogAttribute] = useState<
    ReusableAttribute | null | undefined
  >();
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
  return (
    <PageContainer>
      <PageHeader
        title="Reusable attributes"
        description="Create immutable attribute revisions, publish them, and assemble published revisions into groups."
        actions={
          <Stack direction="row" spacing={1}>
            <Button onClick={() => setGroupOpen(true)} variant="outlined">
              New group
            </Button>
            <Button
              onClick={() => setDialogAttribute(null)}
              startIcon={<AddIcon />}
              variant="contained"
            >
              New attribute
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
        <Typography sx={{ mt: 3 }}>Loading reusable attributes…</Typography>
      ) : (
        <Paper sx={{ mt: 3 }}>
          <Box sx={{ overflowX: 'auto' }}>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>Attribute</TableCell>
                  <TableCell>Version</TableCell>
                  <TableCell>Type</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell align="right">Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {(attributes.data ?? []).map((attribute) => (
                  <TableRow key={attribute.id}>
                    <TableCell>
                      <Typography>{attribute.name}</Typography>
                      <Typography color="text.secondary" variant="caption">
                        {attribute.namespace}:{attribute.code}
                      </Typography>
                    </TableCell>
                    <TableCell>v{attribute.version}</TableCell>
                    <TableCell>{attribute.value_type}</TableCell>
                    <TableCell>
                      <Chip
                        color={
                          attribute.status === 'published'
                            ? 'success'
                            : 'warning'
                        }
                        label={attribute.status}
                        size="small"
                      />
                    </TableCell>
                    <TableCell align="right">
                      <Button
                        onClick={() => setDialogAttribute(attribute)}
                        size="small"
                        startIcon={<EditIcon />}
                      >
                        New revision
                      </Button>
                      {attribute.status === 'draft' && (
                        <Button
                          disabled={publish.isPending}
                          onClick={() => publish.mutate(attribute.id)}
                          size="small"
                          startIcon={<PublishIcon />}
                        >
                          Publish
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
                {attributes.data?.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={5}>
                      No reusable attributes yet.
                    </TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          </Box>
        </Paper>
      )}
      <Typography sx={{ mt: 4 }} variant="h5">
        Groups
      </Typography>
      {groups.isPending ? (
        <Typography sx={{ mt: 2 }}>Loading groups…</Typography>
      ) : (
        <Paper sx={{ mt: 2 }}>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>Group</TableCell>
                <TableCell>Position</TableCell>
                <TableCell>Published revisions</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {(groups.data ?? []).map((group) => (
                <TableRow key={group.id}>
                  <TableCell>
                    <Typography>{group.name}</Typography>
                    <Typography color="text.secondary" variant="caption">
                      {group.code}
                    </Typography>
                  </TableCell>
                  <TableCell>{group.position}</TableCell>
                  <TableCell>
                    {group.reusable_attribute_revision_ids.length}
                  </TableCell>
                </TableRow>
              ))}
              {groups.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={3}>
                    No reusable attribute groups yet.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </Paper>
      )}
      {dialogAttribute !== undefined && (
        <AttributeDialog
          attribute={dialogAttribute ?? undefined}
          onClose={() => setDialogAttribute(undefined)}
        />
      )}
      {groupOpen && (
        <GroupDialog
          attributes={attributes.data ?? []}
          onClose={() => setGroupOpen(false)}
        />
      )}
    </PageContainer>
  );
};
