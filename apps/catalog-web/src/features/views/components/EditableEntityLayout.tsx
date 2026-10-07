import { Box, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../../entities/api';
import { VIEW_EDIT_LAYOUT_SPACING } from '../constants';
import { entityHeadingComponentId } from './blocks/EntityHeadingDefinition';
import { EntityView, type EntityViewProps } from './EntityView';

type SectionHeadingLevel = 'h2' | 'h3';

/** A titled group of fields below an entity's editable layout. */
export const EditableEntitySection = ({
  title,
  headingLevel,
  children,
}: {
  title: string;
  headingLevel: SectionHeadingLevel;
  children: ReactNode;
}) => (
  <Box component="section" sx={{ mt: 4 }}>
    <Typography component={headingLevel} sx={{ mb: 2 }} variant="h6">
      {title}
    </Typography>
    {children}
  </Box>
);

type Props = Omit<
  EntityViewProps,
  'skipComponentId' | 'fallbackVisibilityScope' | 'renderEditor'
> &
  Required<Pick<EntityViewProps, 'renderEditor'>> & {
    /** How the main view lays out fields when it has no layout of its own. */
    fallbackVisibilityScope: EntityViewProps['fallbackVisibilityScope'];
    /** Fields the entity heading displays; they are edited first. */
    headingAttributes: readonly Attribute[];
    /** Editable fields the view leaves out, offered after it. */
    otherAttributes: readonly Attribute[];
    /** The level of section headings, below the page's own heading. */
    headingLevel?: SectionHeadingLevel;
  };

/**
 * An entity's view with every field rendered as an editor: the heading's
 * fields first, then the view without its heading, then fields it omits.
 */
export const EditableEntityLayout = ({
  attributes,
  view,
  fallbackVisibilityScope,
  headingAttributes,
  otherAttributes,
  headingLevel = 'h2',
  ...viewProps
}: Props) => {
  const { t } = useTranslation();
  return (
    <>
      {headingAttributes.length > 0 && (
        <Box sx={{ mb: VIEW_EDIT_LAYOUT_SPACING }}>
          <EntityView {...viewProps} attributes={headingAttributes} />
        </Box>
      )}
      <EntityView
        {...viewProps}
        attributes={attributes}
        fallbackVisibilityScope={fallbackVisibilityScope}
        skipComponentId={entityHeadingComponentId}
        view={view}
      />
      {otherAttributes.length > 0 && (
        <EditableEntitySection
          headingLevel={headingLevel}
          title={t('entities.otherAttributes')}
        >
          <EntityView {...viewProps} attributes={otherAttributes} />
        </EditableEntitySection>
      )}
    </>
  );
};
