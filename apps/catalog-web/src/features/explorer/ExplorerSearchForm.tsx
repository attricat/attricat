import { useForm } from '@tanstack/react-form';
import { Button, MenuItem, Paper, Stack, TextField } from '@mui/material';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { Blueprint } from '../entities/api';
import type { ExplorerSearch } from './search';

type Props = {
  blueprints: Blueprint[];
  search: ExplorerSearch;
  onSubmit: (value: ExplorerSearch) => void;
  lockedBlueprint?: boolean;
};

export const ExplorerSearchForm = ({
  blueprints,
  search,
  onSubmit,
  lockedBlueprint = false,
}: Props) => {
  const { t } = useTranslation();
  const form = useForm({
    defaultValues: {
      blueprint: search.blueprint ?? '',
      query: search.query ?? '',
    },
    onSubmit: ({ value }) => {
      onSubmit({
        blueprint: value.blueprint || undefined,
        version: undefined,
        query: value.query || undefined,
      });
    },
  });

  useEffect(() => {
    form.reset({
      blueprint: search.blueprint ?? '',
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
      sx={{
        mt: 2.5,
        p: 1.5,
        position: 'sticky',
        top: 0,
        zIndex: 2,
      }}
    >
      <Stack direction={{ xs: 'column', md: 'row' }} spacing={1.5}>
        {!lockedBlueprint && (
          <form.Field name="blueprint">
            {(field) => (
              <TextField
                required
                label={t('explorer.selectBlueprint')}
                onChange={(event) => field.handleChange(event.target.value)}
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
        )}
        <form.Field name="query">
          {(field) => (
            <TextField
              fullWidth
              label={t('explorer.query')}
              onChange={(event) => field.handleChange(event.target.value)}
              helperText={t('explorer.queryExamples')}
              placeholder={t('explorer.searchTerms')}
              value={field.state.value}
            />
          )}
        </form.Field>
        <Button type="submit" variant="contained">
          {t('explorer.search')}
        </Button>
      </Stack>
    </Paper>
  );
};
