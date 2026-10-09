import { Box, Tab, Tabs } from '@mui/material';
import { CircleAlertIcon } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { ViewNode } from '../../entities/api';
import { lexiconText } from '../../lexicon/lexicon';
import { nodesPlacedFields } from '../viewFieldComponents';
import { compactIconSize } from '../../../components/iconSizes';

export const ViewTabs = ({
  invalidFields,
  tabs,
  render,
}: {
  /** Fields with errors; tabs holding one are marked, as only one renders. */
  invalidFields?: ReadonlySet<string>;
  tabs: { label: string; children: ViewNode[] }[];
  render: (nodes: ViewNode[]) => ReactNode;
}) => {
  const { t } = useTranslation();
  const [value, setValue] = useState(0);
  const tabId = useId();
  const tabHasErrors = (children: readonly ViewNode[]) =>
    Boolean(invalidFields?.size) &&
    [...nodesPlacedFields(children)].some((code) => invalidFields!.has(code));
  return (
    <>
      <Tabs
        onChange={(_, next) => setValue(next)}
        value={value}
        variant="scrollable"
      >
        {tabs.map((tab, index) => {
          const invalid = tabHasErrors(tab.children);
          return (
            <Tab
              aria-controls={`${tabId}-tabpanel-${index}`}
              aria-description={invalid ? t('views.tabHasErrors') : undefined}
              icon={
                invalid ? <CircleAlertIcon size={compactIconSize} /> : undefined
              }
              iconPosition="end"
              id={`${tabId}-tab-${index}`}
              key={tab.label}
              label={lexiconText(tab.label)}
              sx={invalid ? { color: 'error.main', minHeight: 48 } : undefined}
            />
          );
        })}
      </Tabs>
      {tabs[value] && (
        <Box
          aria-labelledby={`${tabId}-tab-${value}`}
          id={`${tabId}-tabpanel-${value}`}
          role="tabpanel"
          sx={{ pt: 2 }}
        >
          {render(tabs[value].children)}
        </Box>
      )}
    </>
  );
};
