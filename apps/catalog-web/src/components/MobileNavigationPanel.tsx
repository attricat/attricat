import { type ReactNode, useState } from 'react';
import {
  MobileExplorePanelTargetContext,
  SetMobileExplorePanelTargetContext,
} from './mobile-navigation-panel-context';

export const MobileNavigationPanelProvider = ({
  children,
}: {
  children: ReactNode;
}) => {
  const [explorePanelTarget, setExplorePanelTarget] =
    useState<HTMLElement | null>(null);

  return (
    <SetMobileExplorePanelTargetContext.Provider value={setExplorePanelTarget}>
      <MobileExplorePanelTargetContext.Provider value={explorePanelTarget}>
        {children}
      </MobileExplorePanelTargetContext.Provider>
    </SetMobileExplorePanelTargetContext.Provider>
  );
};
