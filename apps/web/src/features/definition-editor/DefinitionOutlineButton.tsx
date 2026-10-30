import { Button, Tooltip } from '@mui/material';
import { ListTreeIcon } from 'lucide-react';
import type { editor } from 'monaco-editor';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';

/** Monaco's list of the document's symbols, filtered as the user types. */
const quickOutlineAction = 'editor.action.quickOutline';

/**
 * Lists a definition's sections, attributes and views to jump to one. It
 * only moves the cursor, so it stays available while the editor is
 * read-only.
 */
export const DefinitionOutlineButton = ({
  editor,
}: {
  /** The mounted editor; the button is disabled until it exists. */
  editor?: editor.IStandaloneCodeEditor;
}) => {
  const { t } = useTranslation();
  return (
    <Tooltip title={t('definitionEditor.outline.shortcut')}>
      <span>
        <Button
          disabled={!editor}
          onClick={() => {
            editor?.focus();
            void editor?.getAction(quickOutlineAction)?.run();
          }}
          startIcon={<ListTreeIcon aria-hidden size={compactIconSize} />}
        >
          {t('definitionEditor.outline.button')}
        </Button>
      </span>
    </Tooltip>
  );
};
