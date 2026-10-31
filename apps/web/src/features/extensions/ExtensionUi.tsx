import { Box } from '@mui/material';
import { lazy, Suspense, type ComponentProps } from 'react';
import {
  defaultExtensionFrameHeight,
  extensionFramePlaceholders,
  renderingModes,
} from './constants';
import { ExtensionFrame } from './ExtensionFrame';
import { ExtensionFrameLoading } from './ExtensionFrameLoading';

// Loaded on demand so native UI and its Worker runtime stay out of the app's
// initial JavaScript.
const NativeUi = lazy(() =>
  import('./native/NativeUi').then(({ NativeUi }) => ({ default: NativeUi })),
);

/**
 * A contribution's UI as its artifact declares: native UI drawn by the host
 * from the extension's Worker, or the extension's own HTML in a frame.
 */
export const ExtensionUi = (props: ComponentProps<typeof ExtensionFrame>) =>
  props.contribution.rendering === renderingModes.native ? (
    <Suspense
      fallback={
        // Holds the same space NativeUi does until it is ready.
        props.placeholder === extensionFramePlaceholders.none ? null : (
          <Box sx={{ minHeight: defaultExtensionFrameHeight }}>
            <ExtensionFrameLoading />
          </Box>
        )
      }
    >
      <NativeUi
        context={props.context}
        contribution={props.contribution}
        onFailure={props.onFailure}
        onReady={props.onReady}
        placeholder={props.placeholder}
        heading={props.heading}
      />
    </Suspense>
  ) : (
    <ExtensionFrame {...props} />
  );
