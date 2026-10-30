import { Button } from '@mui/material';
import { FoldVerticalIcon, UnfoldVerticalIcon } from 'lucide-react';
import type { editor } from 'monaco-editor';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';

/** Monaco's built-in folding actions. */
const foldActions = {
  foldAll: 'editor.foldAll',
  unfoldAll: 'editor.unfoldAll',
} as const;

/**
 * Folds or unfolds every section of a definition. Folding only changes the
 * view, so it stays available while the editor is read-only.
 */
export const DefinitionFoldControls = ({
  editor,
}: {
  /** The mounted editor; the buttons are disabled until it exists. */
  editor?: editor.IStandaloneCodeEditor;
}) => {
  const { t } = useTranslation();
  const run = (action: (typeof foldActions)[keyof typeof foldActions]) => {
    void editor?.getAction(action)?.run();
  };
  return (
    <>
      <Button
        disabled={!editor}
        onClick={() => run(foldActions.foldAll)}
        startIcon={<FoldVerticalIcon aria-hidden size={compactIconSize} />}
      >
        {t('definitionEditor.fold.foldAll')}
      </Button>
      <Button
        disabled={!editor}
        onClick={() => run(foldActions.unfoldAll)}
        startIcon={<UnfoldVerticalIcon aria-hidden size={compactIconSize} />}
      >
        {t('definitionEditor.fold.unfoldAll')}
      </Button>
    </>
  );
};
