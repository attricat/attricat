import { Link } from '@tanstack/react-router';
import type { CSSProperties } from 'react';
import { lexiconText } from '../lexicon/lexicon';

/** A blueprint's name, linking to its management page. */
export const BlueprintLink = ({
  blueprint,
  style,
}: {
  blueprint: { id: string; name: string };
  style?: CSSProperties;
}) => (
  <Link
    params={{ blueprintId: blueprint.id }}
    style={style}
    to="/manage/blueprints/$blueprintId"
  >
    {lexiconText(blueprint.name)}
  </Link>
);
