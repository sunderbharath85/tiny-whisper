import { Component, type ErrorInfo, type ReactNode } from "react";
import { Button } from "@/components/ui/button";

type State = { error: Error | null; componentStack: string };

/// Catches render errors and shows the full-screen fatal view. Failures in
/// event handlers and promises don't get here; they go to the error area.
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null, componentStack: "" };

  static getDerivedStateFromError(error: unknown): Partial<State> {
    return { error: error instanceof Error ? error : new Error(String(error)) };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    console.error("Render error:", error, info.componentStack);
    this.setState({ componentStack: info.componentStack ?? "" });
  }

  render() {
    const { error, componentStack } = this.state;
    if (!error) return this.props.children;
    return (
      <div role="alert" className="p-6 text-sm space-y-3">
        <div className="font-semibold">Tiny Whisper crashed</div>
        <pre className="whitespace-pre-wrap text-xs text-[var(--color-destructive)]">
          {`${error.stack ?? error.message}${componentStack}`}
        </pre>
        <Button variant="outline" size="sm" onClick={() => window.location.reload()}>
          Reload window
        </Button>
      </div>
    );
  }
}
