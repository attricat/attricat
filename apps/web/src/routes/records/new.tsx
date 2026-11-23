import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';
import { CreateRecordPage } from '../../features/records/CreateRecordPage';
import { RequireCapability } from '../../features/auth/RequireCapability';
import { relationshipPickerModes } from '../../features/records/constants';
import { capabilities } from '../../features/auth/capabilities';

const NewRecordRouteComponent = () => (
  <RequireCapability capability={capabilities.recordsWrite}>
    <CreateRecordPage search={Route.useSearch()} />
  </RequireCapability>
);

export const Route = createFileRoute('/records/new')({
  validateSearch: z.object({
    blueprint: z.string().trim().min(1).optional().catch(undefined),
    locked: z.boolean().optional().catch(undefined),
    /**
     * A relationship picker's Explorer tab that the form returns to with
     * the new record selected.
     */
    relationshipPicker: z.uuid().optional().catch(undefined),
    relationshipPickerMode: z
      .enum(relationshipPickerModes)
      .optional()
      .catch(undefined),
    /** Relationship values the form starts with, by attribute code. */
    values: z.record(z.string(), z.array(z.uuid())).optional().catch(undefined),
  }),
  component: NewRecordRouteComponent,
});
