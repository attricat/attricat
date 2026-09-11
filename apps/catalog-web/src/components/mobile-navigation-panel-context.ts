import {
  createContext,
  type Dispatch,
  type SetStateAction,
  useContext,
} from 'react';

export const MobileExplorePanelTargetContext =
  createContext<HTMLElement | null>(null);
export const SetMobileExplorePanelTargetContext = createContext<
  Dispatch<SetStateAction<HTMLElement | null>>
>(() => undefined);

export const useMobileExplorePanelTarget = () =>
  useContext(MobileExplorePanelTargetContext);

export const useSetMobileExplorePanelTarget = () =>
  useContext(SetMobileExplorePanelTargetContext);
