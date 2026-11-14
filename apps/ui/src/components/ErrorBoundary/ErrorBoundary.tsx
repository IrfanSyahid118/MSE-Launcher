import { Component } from 'react'
import type { ErrorInfo, ReactNode } from 'react'

export interface ErrorBoundaryProps {
  children: ReactNode
  fallback: ReactNode | ((reset: () => void) => ReactNode)
  onError?: (error: Error, info: ErrorInfo) => void
}

interface ErrorBoundaryState {
  failed: boolean
}

/*
 * Menahan kegagalan render satu komponen supaya sisa aplikasi tetap hidup.
 * Renderer React yang gagal total akan mengosongkan jendela tanpa boundary.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { failed: false }

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { failed: true }
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    this.props.onError?.(error, info)
  }

  private readonly reset = (): void => {
    this.setState({ failed: false })
  }

  override render(): ReactNode {
    if (!this.state.failed) {
      return this.props.children
    }
    const { fallback } = this.props
    return typeof fallback === 'function' ? fallback(this.reset) : fallback
  }
}
