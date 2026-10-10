import { Box, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../../records/api';
import { VIEW_EDIT_LAYOUT_SPACING } from '../constants';
import { recordHeadingComponentId } from './blocks/RecordHeadingDefinition';
import { RecordView, type RecordViewProps } from './RecordView';

type SectionHeadingLevel = 'h2' | 'h3';

/** A titled group of fields below a record's editable layout. */
export const EditableRecordSection = ({
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
  RecordViewProps,
  'skipComponentId' | 'fallbackVisibilityScope' | 'renderEditor'
> &
  Required<Pick<RecordViewProps, 'renderEditor'>> & {
    /** How the main view lays out fields when it has no layout of its own. */
    fallbackVisibilityScope: RecordViewProps['fallbackVisibilityScope'];
    /** Fields the record heading displays; they are edited first. */
    headingAttributes: readonly Attribute[];
    /** Editable fields the view leaves out, offered after it. */
    otherAttributes: readonly Attribute[];
    /** The level of section headings, below the page's own heading. */
    headingLevel?: SectionHeadingLevel;
  };

/**
 * A record's view with every field rendered as an editor: the heading's
 * fields first, then the view without its heading, then fields it omits.
 */
export const EditableRecordLayout = ({
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
          <RecordView {...viewProps} attributes={headingAttributes} />
        </Box>
      )}
      <RecordView
        {...viewProps}
        attributes={attributes}
        fallbackVisibilityScope={fallbackVisibilityScope}
        skipComponentId={recordHeadingComponentId}
        view={view}
      />
      {otherAttributes.length > 0 && (
        <EditableRecordSection
          headingLevel={headingLevel}
          title={t('records.otherAttributes')}
        >
          <RecordView {...viewProps} attributes={otherAttributes} />
        </EditableRecordSection>
      )}
    </>
  );
};
