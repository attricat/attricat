import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Typography,
} from '@mui/material';
import { ChevronDownIcon, CircleAlertIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { ViewNode } from '../../records/api';
import { lexiconText } from '../../lexicon/lexicon';
import { compactIconSize } from '../../../components/iconSizes';
import { useFieldRevealRequest } from '../fieldReveal';
import { nodesPlacedFields } from '../viewFieldComponents';

/**
 * One accordion section of a view. It opens for a field the user asks to see
 * and, like a tab, is marked while it holds a field with an error.
 */
export const ViewAccordionSection = ({
  invalidFields,
  label,
  nodes,
  children,
}: {
  invalidFields?: ReadonlySet<string>;
  label: string;
  nodes: readonly ViewNode[];
  children: ReactNode;
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  useFieldRevealRequest((code) => {
    if (nodesPlacedFields(nodes).has(code)) setExpanded(true);
  });
  const invalid =
    Boolean(invalidFields?.size) &&
    [...nodesPlacedFields(nodes)].some((code) => invalidFields!.has(code));
  return (
    <Accordion expanded={expanded} onChange={(_, next) => setExpanded(next)}>
      <AccordionSummary
        aria-description={invalid ? t('views.tabHasErrors') : undefined}
        expandIcon={<ChevronDownIcon />}
      >
        <Box
          sx={{
            alignItems: 'center',
            color: invalid ? 'error.main' : undefined,
            display: 'flex',
            gap: 1.5,
          }}
        >
          <Typography color="inherit">{lexiconText(label)}</Typography>
          {invalid && <CircleAlertIcon aria-hidden size={compactIconSize} />}
        </Box>
      </AccordionSummary>
      <AccordionDetails>{children}</AccordionDetails>
    </Accordion>
  );
};
