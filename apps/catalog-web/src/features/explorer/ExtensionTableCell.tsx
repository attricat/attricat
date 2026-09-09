import { Box } from '@mui/material';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { z } from 'zod';
import { ExtensionFrame } from '../extensions/ExtensionFrame';
import type { ExtensionContribution } from '../extensions/api';
import { recordFrameTiming } from '../inspector/timing';

export const explorerTableCellContextSchema = z
  .object({
    context_version: z.literal(1),
    column: z
      .object({
        field: z.string().min(1),
        label: z.string().nullable(),
        renderer: z
          .object({
            id: z.string().min(1),
            version: z.number().int().positive(),
            props: z.record(z.string(), z.unknown()),
          })
          .strict(),
      })
      .strict(),
    primary_value: z.unknown(),
    related_entity: z
      .object({
        id: z.uuid(),
        blueprint_id: z.uuid(),
        blueprint_version: z.number().int().positive(),
        relationship_context_id: z.uuid(),
        relationship_context_code: z.string(),
      })
      .strict()
      .nullable(),
    related_preview: z.record(z.string(), z.unknown()).nullable(),
    source_row: z
      .object({
        entity_id: z.uuid(),
        blueprint_version: z.number().int().positive(),
        preview: z.record(z.string(), z.unknown()),
      })
      .strict(),
  })
  .strict();

const cellStartTimeout = 1_500;

type Props = {
  contribution?: ExtensionContribution;
  context: z.infer<typeof explorerTableCellContextSchema>;
  fallback: ReactNode;
  frameAllowed: boolean;
};

/**
 * A cell never delays the Explorer: the normal scalar is shown until its
 * sandbox has started, and permanently if the renderer is unavailable, slow,
 * or fails. The parent only creates this component for virtualized cells.
 */
export const ExtensionTableCell = ({
  contribution,
  context,
  fallback,
  frameAllowed,
}: Props) => {
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const startedAt = useRef(performance.now());
  useEffect(() => {
    if (!contribution || !frameAllowed || ready || failed) return;
    const timer = window.setTimeout(() => setFailed(true), cellStartTimeout);
    return () => window.clearTimeout(timer);
  }, [contribution, failed, frameAllowed, ready]);

  const handleFailure = useCallback(() => {
    setFailed((wasFailed) => {
      if (!wasFailed)
        recordFrameTiming('frame-fallback', performance.now() - startedAt.current);
      return true;
    });
  }, []);
  const handleReady = useCallback(() => {
    setReady((wasReady) => {
      if (!wasReady)
        recordFrameTiming('frame-load', performance.now() - startedAt.current);
      return true;
    });
  }, []);
  if (!contribution || !frameAllowed || failed) return <>{fallback}</>;
  return (
    <Box sx={{ minWidth: 0 }}>
      {!ready && fallback}
      <Box
        sx={
          ready
            ? undefined
            : { height: 0, overflow: 'hidden', visibility: 'hidden' }
        }
      >
        <ExtensionFrame
          context={context}
          contribution={contribution}
          onFailure={handleFailure}
          onReady={handleReady}
        />
      </Box>
    </Box>
  );
};
