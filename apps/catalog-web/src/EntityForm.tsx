import { useForm } from '@tanstack/react-form'
import {
  Alert,
  Button,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material'
import type { BlueprintWithAttributes } from './api'
import {
  relationshipTargetsForForm,
  serializeAttributeValues,
  valuesForForm,
} from './entity-form'

type EntityFormProps = {
  blueprint?: BlueprintWithAttributes
  initialValues?: ReturnType<typeof valuesForForm>
  isLoadingBlueprint?: boolean
  error?: Error | null
  onLoadBlueprint?: (code: string, version?: number) => void
  onSubmit: (input: {
    values: ReturnType<typeof serializeAttributeValues>
    relationships: ReturnType<typeof relationshipTargetsForForm>
  }) => void
  submitLabel: string
}

export function EntityForm({
  blueprint,
  initialValues = {},
  isLoadingBlueprint = false,
  error,
  onLoadBlueprint,
  onSubmit,
  submitLabel,
}: EntityFormProps) {
  const form = useForm({
    defaultValues: {
      blueprintCode: blueprint?.blueprint.code ?? '',
      blueprintVersion: blueprint?.blueprint.version.toString() ?? '',
      fields: initialValues,
    },
    onSubmit: ({ value }) => {
      if (!blueprint && onLoadBlueprint) {
        const version = Number(value.blueprintVersion)
        onLoadBlueprint(
          value.blueprintCode.trim(),
          Number.isInteger(version) && version > 0 ? version : undefined,
        )
        return
      }
      if (blueprint) {
        onSubmit({
          values: serializeAttributeValues(blueprint.attributes, value.fields),
          relationships: relationshipTargetsForForm(
            blueprint.attributes,
            value.fields,
          ),
        })
      }
    },
  })

  return (
    <Paper
      component="form"
      onSubmit={(event) => {
        event.preventDefault()
        void form.handleSubmit()
      }}
      sx={{ mt: 4, p: 3 }}
    >
      <Stack spacing={2}>
        {!blueprint && (
          <>
            <Typography variant="h6">Choose blueprint</Typography>
            <form.Field name="blueprintCode">
              {(field) => (
                <TextField
                  required
                  label="Blueprint code"
                  onChange={(event) => field.handleChange(event.target.value)}
                  placeholder="product"
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="blueprintVersion">
              {(field) => (
                <TextField
                  inputMode="numeric"
                  label="Blueprint version"
                  onChange={(event) => field.handleChange(event.target.value)}
                  placeholder="Current"
                  value={field.state.value}
                />
              )}
            </form.Field>
          </>
        )}
        {blueprint && (
          <Typography color="text.secondary">
            {blueprint.blueprint.code} v{blueprint.blueprint.version}
          </Typography>
        )}
        {blueprint && (
          <form.Field name="fields">
            {(field) => (
              <>
                {blueprint.attributes.map((attribute) => (
                  <TextField
                    key={attribute.code}
                    fullWidth
                    label={attribute.code}
                    helperText={
                      attribute.value_type === 'relationship'
                        ? 'Comma-separated entity UUIDs'
                        : undefined
                    }
                    onChange={(event) =>
                      field.handleChange({
                        ...field.state.value,
                        [attribute.code]: event.target.value,
                      })
                    }
                    value={field.state.value[attribute.code] ?? ''}
                  />
                ))}
              </>
            )}
          </form.Field>
        )}
        {error && <Alert severity="error">{error.message}</Alert>}
        <Button disabled={isLoadingBlueprint} type="submit" variant="contained">
          {isLoadingBlueprint ? 'Loading blueprint...' : submitLabel}
        </Button>
      </Stack>
    </Paper>
  )
}
