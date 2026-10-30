import type { Monaco } from '@monaco-editor/react';
import { Button, Tooltip } from '@mui/material';
import { CircleAlertIcon, TriangleAlertIcon } from 'lucide-react';
import type { editor, Uri } from 'monaco-editor';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { serverDiagnosticsOwner } from './constants';

/** Monaco's action that shows the next problem below its line. */
const nextProblemAction = 'editor.action.marker.next';

type ProblemCounts = { errors: number; warnings: number };

const countProblems = (monaco: Monaco, model: editor.ITextModel) => {
  // The server's check has its own status beside the toolbar.
  const markers = monaco.editor
    .getModelMarkers({ resource: model.uri })
    .filter(({ owner }: editor.IMarker) => owner !== serverDiagnosticsOwner);
  const { MarkerSeverity } = monaco;
  return {
    errors: markers.filter(
      ({ severity }: editor.IMarker) => severity === MarkerSeverity.Error,
    ).length,
    warnings: markers.filter(
      ({ severity }: editor.IMarker) => severity === MarkerSeverity.Warning,
    ).length,
  };
};

/**
 * How many problems the editor marks in a definition; each click shows the
 * next one with its explanation. Hidden while there are none.
 */
export const DefinitionProblemsButton = ({
  mounted,
}: {
  mounted?: { editor: editor.IStandaloneCodeEditor; monaco: Monaco };
}) => {
  const { t } = useTranslation();
  const [counts, setCounts] = useState<ProblemCounts>({
    errors: 0,
    warnings: 0,
  });
  useEffect(() => {
    const model = mounted?.editor.getModel();
    if (!mounted || !model) return;
    const update = () => setCounts(countProblems(mounted.monaco, model));
    update();
    const subscription = mounted.monaco.editor.onDidChangeMarkers(
      (uris: readonly Uri[]) => {
        if (uris.some((uri) => uri.toString() === model.uri.toString()))
          update();
      },
    );
    return () => subscription.dispose();
  }, [mounted]);
  const count = counts.errors + counts.warnings;
  if (!mounted || count === 0) return null;
  const Icon = counts.errors > 0 ? CircleAlertIcon : TriangleAlertIcon;
  return (
    <Tooltip describeChild title={t('definitionEditor.problems.next')}>
      <Button
        color={counts.errors > 0 ? 'error' : 'warning'}
        onClick={() => {
          mounted.editor.focus();
          void mounted.editor.getAction(nextProblemAction)?.run();
        }}
        startIcon={<Icon aria-hidden size={compactIconSize} />}
      >
        {t('definitionEditor.problems.count', { count })}
      </Button>
    </Tooltip>
  );
};
