import { z } from 'zod';
import { FILE_STATUS_VALUES } from './constants';

export const fileVariantSchema = z.object({
  kind: z.string(),
  mime_type: z.string(),
  width: z.number().int().nullable(),
  height: z.number().int().nullable(),
  byte_size: z.number().int().nonnegative(),
  sha256: z.string(),
});

export const fileMetadataSchema = z.object({
  id: z.uuid(),
  filename: z.string(),
  mime_type: z.string(),
  byte_size: z.number().int().nonnegative(),
  sha256: z.string(),
  status: z.enum(FILE_STATUS_VALUES),
  variants: z.array(fileVariantSchema),
});

const uploadedFilesSchema = z.array(
  fileMetadataSchema
    .omit({ variants: true })
    .extend({ variants: z.array(fileVariantSchema).default([]) }),
);

export const fileUploadResultSchema = z.object({
  attribute_code: z.string(),
  context_id: z.uuid(),
  files: uploadedFilesSchema,
});
export const conversationUploadResultSchema = z.object({
  files: uploadedFilesSchema,
});

export const updateFileReferencesSchema = z.object({
  context_id: z.uuid().nullable(),
  expected_file_ids: z.array(z.uuid()),
  file_ids: z.array(z.uuid()),
});

export const fileReferencesUpdateResultSchema = z.object({
  entity_updated_at: z.string(),
});

export type FileMetadata = z.infer<typeof fileMetadataSchema>;
export type FileUploadResult = z.infer<typeof fileUploadResultSchema>;
