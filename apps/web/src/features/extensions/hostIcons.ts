import {
  CalculatorIcon,
  CheckIcon,
  ClockIcon,
  DownloadIcon,
  FileIcon,
  InfoIcon,
  LinkIcon,
  RefreshCwIcon,
  SearchIcon,
  SendIcon,
  SparklesIcon,
  TagIcon,
  TriangleAlertIcon,
  WandIcon,
  type LucideIcon,
} from 'lucide-react';
import { createElement } from 'react';
import { ExtensionIcon, ManagementIcon } from '../../components/systemIcons';
import type { HostIcon } from './constants';

/** The host's drawing of each icon an extension may name. */
const hostIconComponents: Record<HostIcon, LucideIcon> = {
  alert: TriangleAlertIcon,
  calculator: CalculatorIcon,
  check: CheckIcon,
  clock: ClockIcon,
  download: DownloadIcon,
  file: FileIcon,
  info: InfoIcon,
  link: LinkIcon,
  refresh: RefreshCwIcon,
  search: SearchIcon,
  send: SendIcon,
  settings: ManagementIcon,
  sparkles: SparklesIcon,
  tag: TagIcon,
  wand: WandIcon,
};

/** The icon an extension named, or the generic extension icon, drawn at `size`. */
export const renderHostIcon = (
  icon: HostIcon | null | undefined,
  size: number,
) => createElement(icon ? hostIconComponents[icon] : ExtensionIcon, { size });
