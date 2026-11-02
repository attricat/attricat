import { Accordion, AccordionDetails, AccordionSummary } from '@mui/material';
import type { ReactNode } from 'react';

/** Detail inside a conversation, folded away until asked for. */
export const DetailsAccordion = ({
  children,
  title,
}: {
  children: ReactNode;
  title: ReactNode;
}) => (
  <Accordion disableGutters elevation={0} sx={{ bgcolor: 'action.hover' }}>
    <AccordionSummary>{title}</AccordionSummary>
    <AccordionDetails>{children}</AccordionDetails>
  </Accordion>
);
