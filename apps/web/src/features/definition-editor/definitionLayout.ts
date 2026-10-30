import { useState } from 'react';
import { z } from 'zod';
import {
  readStoredJson,
  storageAreas,
  writeStoredJson,
} from '../../components/browserStorage';

/** How much of the workspace's width the editor takes beside the preview. */
export const editorShareLimits = { default: 0.6, max: 0.8, min: 0.3 } as const;
/** How far one arrow key press moves the divider. */
export const editorShareStep = 0.05;

// Remembers how the user arranged definition editors, for every editor.
const storageKey = 'attricat.definition-editor-layout';
const layoutSchema = z.object({
  editorShare: z.number().min(editorShareLimits.min).max(editorShareLimits.max),
  previewHidden: z.boolean(),
});
type DefinitionLayout = z.infer<typeof layoutSchema>;

export const clampEditorShare = (share: number) =>
  Math.min(editorShareLimits.max, Math.max(editorShareLimits.min, share));

/**
 * The editor's share of the width and whether the preview is hidden, kept
 * across visits. `resize` only previews a width; `commitResize` keeps it.
 */
export const useDefinitionLayout = () => {
  const [layout, setLayout] = useState<DefinitionLayout>(
    () =>
      readStoredJson(storageAreas.local, storageKey, layoutSchema) ?? {
        editorShare: editorShareLimits.default,
        previewHidden: false,
      },
  );
  const save = (next: DefinitionLayout) => {
    setLayout(next);
    writeStoredJson(storageAreas.local, storageKey, next);
  };
  return {
    ...layout,
    commitResize: (share: number) =>
      save({ ...layout, editorShare: clampEditorShare(share) }),
    resize: (share: number) =>
      setLayout({ ...layout, editorShare: clampEditorShare(share) }),
    togglePreview: () =>
      save({ ...layout, previewHidden: !layout.previewHidden }),
  };
};
