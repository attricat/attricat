import { useId } from 'react';

/** Keep each tab's control and its panel's label in sync. */
export const useTabAccessibility = () => {
  const id = useId();
  const tab = (index: number) => ({
    'aria-controls': `${id}-panel-${index}`,
    id: `${id}-tab-${index}`,
  });
  const panel = (index: number) => ({
    'aria-labelledby': `${id}-tab-${index}`,
    id: `${id}-panel-${index}`,
    role: 'tabpanel' as const,
  });
  return { tab, panel };
};
