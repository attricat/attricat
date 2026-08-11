import { useMutation } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { createEntity, getBlueprintByCode } from '../../api'
import { EntityForm } from '../../EntityForm'
import { EntityPage } from '../../EntityPage'

export const Route = createFileRoute('/entities/new')({ component: NewEntity })

function NewEntity() {
  const navigate = useNavigate({ from: Route.fullPath })
  const blueprint = useMutation({
    mutationFn: ({ code, version }: { code: string; version?: number }) =>
      getBlueprintByCode(code, version),
  })
  const create = useMutation({
    mutationFn: ({
      values,
      relationships,
    }: {
      values: Parameters<typeof createEntity>[0]['values']
      relationships: { attribute_code: string; target_entity_ids: string[] }[]
    }) => {
      const resolved = blueprint.data
      if (!resolved)
        throw new Error('Choose a blueprint before creating an entity')
      return createEntity({
        blueprint: {
          code: resolved.blueprint.code,
          version: resolved.blueprint.version,
        },
        values: [
          ...values,
          ...relationships.flatMap((relationship) =>
            relationship.target_entity_ids.map((target_entity_id) => ({
              kind: 'relationship' as const,
              attribute_code: relationship.attribute_code,
              target_entity_id,
            })),
          ),
        ],
      })
    },
    onSuccess: (entity) => {
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      })
    },
  })
  return (
    <EntityPage title="Create entity">
      <EntityForm
        blueprint={blueprint.data}
        error={blueprint.error ?? create.error}
        isLoadingBlueprint={blueprint.isPending || create.isPending}
        onLoadBlueprint={(code, version) => blueprint.mutate({ code, version })}
        onSubmit={(input) => create.mutate(input)}
        submitLabel={blueprint.data ? 'Create entity' : 'Load blueprint'}
      />
    </EntityPage>
  )
}
