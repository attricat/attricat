import { z } from 'zod';

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
  status: z.enum([
    'uploading',
    'queued',
    'processing',
    'ready',
    'failed',
    'deleted',
  ]),
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

export type FileMetadata = z.infer<typeof fileMetadataSchema>;
export type FileUploadResult = z.infer<typeof fileUploadResultSchema>;
