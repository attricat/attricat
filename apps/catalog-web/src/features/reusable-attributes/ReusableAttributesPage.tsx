import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { RouterButton } from '../../components/RouterLink';
import AddIcon from '@mui/icons-material/Add';
import PublishIcon from '@mui/icons-material/Publish';
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
  createReusableAttributeGroup,
  listReusableAttributeGroups,
  listReusableAttributes,
  publishReusableAttributeRevision,
  type ReusableAttribute,
} from './api';
import { latestReusableAttributeRevisions } from './latestRevisions';
import { reusableAttributeQueryKeys } from './queryKeys';

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
        !value.revisionIds.length
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
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={() => {
        if (!save.isPending) onClose();
      }}
      open
    >
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
                  label="Position"
                  onChange={(event) => field.handleChange(event.target.value)}
                  slotProps={{ htmlInput: { min: 0, step: 1 } }}
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
                  select
                  slotProps={{ select: { multiple: true } }}
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
          <Button disabled={save.isPending} onClick={onClose}>
            Cancel
          </Button>
          <Button
            disabled={save.isPending || !published.length}
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
        description="Create immutable attribute revisions, publish them, and assemble published revisions into groups."
        title="Reusable attributes"
        actions={
          <Stack direction="row" spacing={1}>
            <Button onClick={() => setGroupOpen(true)} variant="outlined">
              New group
            </Button>
            <Button
              onClick={() =>
                navigate({ to: '/manage/reusable-attributes/new' })
              }
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
                {displayedAttributes.map((attribute) => (
                  <TableRow hover key={attribute.definition_id}>
                    <TableCell>
                      <Link
                        params={{ definitionId: attribute.definition_id }}
                        to="/manage/reusable-attributes/$definitionId"
                      >
                        <Typography>{attribute.name}</Typography>
                        <Typography color="text.secondary" variant="caption">
                          {attribute.namespace}:{attribute.code}
                        </Typography>
                      </Link>
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
                      <RouterButton
                        params={{ definitionId: attribute.definition_id }}
                        size="small"
                        to="/manage/reusable-attributes/$definitionId"
                      >
                        Edit
                      </RouterButton>
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
                {!displayedAttributes.length && (
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
              {!groups.data?.length && (
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
      {groupOpen && (
        <GroupDialog
          attributes={attributes.data ?? []}
          onClose={() => setGroupOpen(false)}
        />
      )}
    </PageContainer>
  );
};
