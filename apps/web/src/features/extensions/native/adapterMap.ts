import type { ComponentType } from 'react';
import {
  BadgeAdapter,
  ButtonAdapter,
  CheckboxAdapter,
  DividerAdapter,
  EmptyStateAdapter,
  GridAdapter,
  HeadingAdapter,
  IconButtonAdapter,
  LinkAdapter,
  NumberFieldAdapter,
  SelectAdapter,
  StackAdapter,
  SwitchAdapter,
  TableAdapter,
  TextAdapter,
  TextFieldAdapter,
  type AdapterProps,
} from './adapters';
import type { UiKitElementName } from './generated/uiKit';

/** The host's drawing of every UI kit element. */
export const nativeAdapters: Record<
  UiKitElementName,
  ComponentType<AdapterProps>
> = {
  Stack: StackAdapter,
  Grid: GridAdapter,
  Divider: DividerAdapter,
  Text: TextAdapter,
  Heading: HeadingAdapter,
  Button: ButtonAdapter,
  IconButton: IconButtonAdapter,
  TextField: TextFieldAdapter,
  NumberField: NumberFieldAdapter,
  Select: SelectAdapter,
  Checkbox: CheckboxAdapter,
  Switch: SwitchAdapter,
  Badge: BadgeAdapter,
  Link: LinkAdapter,
  Table: TableAdapter,
  EmptyState: EmptyStateAdapter,
};
