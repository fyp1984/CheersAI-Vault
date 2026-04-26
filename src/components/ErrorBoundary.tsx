import React from 'react';

interface Props {
  children: React.ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
}

export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex items-center justify-center h-full">
          <div className="text-center p-8 max-w-2xl">
            <h1 className="text-2xl font-bold text-red-600 mb-4">页面加载出错</h1>
            <p className="text-gray-700 mb-4">抱歉，页面加载时发生了错误。</p>
            {this.state.error && (
              <div className="bg-red-50 border border-red-200 rounded p-4 text-left">
                <p className="font-mono text-sm text-red-800">{this.state.error.toString()}</p>
                {this.state.error.stack && (
                  <pre className="mt-2 text-xs text-red-700 overflow-auto">
                    {this.state.error.stack}
                  </pre>
                )}
              </div>
            )}
            <button
              onClick={() => window.location.reload()}
              className="mt-4 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
            >
              重新加载
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
