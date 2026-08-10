import type { ReactNode } from "react";
import { Component } from "react";

import i18n from "@/lib/i18n";

interface ErrorBoundaryProps {
  children: ReactNode;
  fallbackTitle?: string;
  fallbackDescription?: string;
}

interface ErrorBoundaryState {
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = {
    error: null,
  };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, errorInfo: { componentStack: string }) {
    console.error("UI render crashed:", error, errorInfo.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
          <p className="font-medium">
            {this.props.fallbackTitle ?? i18n.t("common:renderFailedTitle")}
          </p>
          <p className="mt-1 text-xs leading-5">
            {this.props.fallbackDescription ?? i18n.t("common:renderFailedDescription")}
          </p>
          <pre className="mt-3 overflow-x-auto whitespace-pre-wrap rounded-md bg-background/70 p-3 text-[11px] leading-5 text-foreground">
            {this.state.error.stack || this.state.error.message}
          </pre>
        </div>
      );
    }

    return this.props.children;
  }
}
