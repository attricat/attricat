import { Box, Tab, Tabs } from '@mui/material';
import { useId, useState, type ReactNode } from 'react';
import type { ViewNode } from '../../entities/api';

export const ViewTabs = ({
  tabs,
  render,
}: {
  tabs: { label: string; children: ViewNode[] }[];
  render: (nodes: ViewNode[]) => ReactNode;
}) => {
  const [value, setValue] = useState(0);
  const tabId = useId();
  return (
    <>
      <Tabs
        onChange={(_, next) => setValue(next)}
        value={value}
        variant="scrollable"
      >
        {tabs.map((tab, index) => (
          <Tab
            aria-controls={`${tabId}-tabpanel-${index}`}
            id={`${tabId}-tab-${index}`}
            key={tab.label}
            label={tab.label}
          />
        ))}
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
