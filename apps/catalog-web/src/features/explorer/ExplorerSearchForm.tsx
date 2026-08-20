import { useForm } from '@tanstack/react-form';
import { Button, MenuItem, Paper, Stack, TextField } from '@mui/material';
import { useEffect } from 'react';
import type { Blueprint } from '../entities/api';
import type { ExplorerSearch } from './search';

type Props = {
  blueprints: Blueprint[];
  search: ExplorerSearch;
  onSubmit: (value: ExplorerSearch) => void;
};

export const ExplorerSearchForm = ({ blueprints, search, onSubmit }: Props) => {
  const form = useForm({
    defaultValues: {
      blueprint: search.blueprint ?? '',
      version: search.version?.toString() ?? '',
      query: search.query ?? '',
    },
    onSubmit: ({ value }) => {
      onSubmit({
        blueprint: value.blueprint || undefined,
        version: value.version ? Number(value.version) : undefined,
        query: value.query || undefined,
      });
    },
  });

  useEffect(() => {
    form.reset({
      blueprint: search.blueprint ?? '',
      version: search.version?.toString() ?? '',
      query: search.query ?? '',
    });
  }, [form, search.blueprint, search.query, search.version]);

  return (
    <Paper
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
      sx={{ mt: 4, p: 2.5 }}
    >
      <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}>
        <form.Field name="blueprint">
          {(field) => (
            <TextField
              required
              label="Select a Blueprint"
              onChange={(event) => {
                field.handleChange(event.target.value);
                form.setFieldValue('version', '');
              }}
              select
              sx={{ width: 280 }}
              value={field.state.value}
            >
              {blueprints.map((blueprint) => (
                <MenuItem key={blueprint.code} value={blueprint.code}>
                  {blueprint.name} ({blueprint.code})
                </MenuItem>
              ))}
            </TextField>
          )}
        </form.Field>
        <form.Field name="version">
          {(field) => (
            <TextField
              inputMode="numeric"
              label="Version"
              onChange={(event) => field.handleChange(event.target.value)}
              placeholder="Current"
              value={field.state.value}
            />
          )}
        </form.Field>
        <form.Field name="query">
          {(field) => (
            <TextField
              fullWidth
              label="Query"
              onChange={(event) => field.handleChange(event.target.value)}
              placeholder="Search terms"
              value={field.state.value}
            />
          )}
        </form.Field>
        <Button type="submit" variant="contained">
          Search
        </Button>
      </Stack>
    </Paper>
  );
};
