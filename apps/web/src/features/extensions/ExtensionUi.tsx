import type { ComponentProps } from 'react';
import { renderingModes } from './constants';
import { ExtensionFrame } from './ExtensionFrame';
import { NativeUi } from './native/NativeUi';

/**
 * A contribution's UI as its artifact declares: native UI drawn by the host
 * from the extension's Worker, or the extension's own HTML in a frame.
 */
export const ExtensionUi = (props: ComponentProps<typeof ExtensionFrame>) =>
  props.contribution.rendering === renderingModes.native ? (
    <NativeUi
      context={props.context}
      contribution={props.contribution}
      onFailure={props.onFailure}
      onReady={props.onReady}
      placeholder={props.placeholder}
      heading={props.heading}
    />
  ) : (
    <ExtensionFrame {...props} />
  );
