import { Alert } from '@mui/material';
import { Component, type ErrorInfo, type ReactNode } from 'react';

type Props = {
  children: ReactNode;
  fallbackMessage: string;
  logLabel: string;
};

type State = { failed: boolean };

export class FieldErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  static getDerivedStateFromError = (): State => ({ failed: true });

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`Could not render ${this.props.logLabel}`, error, info);
  }

  render() {
    if (this.state.failed) {
      return <Alert severity="warning">{this.props.fallbackMessage}</Alert>;
    }
    return this.props.children;
  }
}
