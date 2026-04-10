import { useState } from 'react'
import { useMutation } from '@tanstack/react-query'
import { executeScript } from '../../api'
import type { ExecuteScriptResponse } from '../../types'

interface DryRunPanelProps {
  script: string
  timeoutSeconds: number
}

const WARNING_KEY = 'prodzilla-dryrun-warned'

export default function DryRunPanel({ script, timeoutSeconds }: DryRunPanelProps) {
  const [showWarning, setShowWarning] = useState(false)
  const [collapsed, setCollapsed] = useState(false)
  const [result, setResult] = useState<ExecuteScriptResponse | null>(null)

  const mutation = useMutation({
    mutationFn: () => executeScript(script, timeoutSeconds),
    onSuccess: (data) => setResult(data),
  })

  function handleRun() {
    if (!sessionStorage.getItem(WARNING_KEY)) {
      setShowWarning(true)
      return
    }
    mutation.mutate()
  }

  function confirmRun() {
    sessionStorage.setItem(WARNING_KEY, '1')
    setShowWarning(false)
    mutation.mutate()
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handleRun}
          disabled={mutation.isPending || !script.trim()}
          className="px-3 py-1.5 text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 rounded-md disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {mutation.isPending ? 'Running...' : 'Run Script'}
        </button>
        {result && (
          <button
            type="button"
            onClick={() => setCollapsed(!collapsed)}
            className="text-xs text-gray-500 hover:text-gray-700"
          >
            {collapsed ? 'Show output' : 'Hide output'}
          </button>
        )}
      </div>

      {showWarning && (
        <div className="border border-yellow-300 bg-yellow-50 rounded-md p-3 text-sm">
          <p className="font-medium text-yellow-800">
            Scripts execute real HTTP requests against configured URLs.
          </p>
          <div className="mt-2 flex gap-2">
            <button
              type="button"
              onClick={confirmRun}
              className="px-2 py-1 text-xs font-medium text-white bg-yellow-600 hover:bg-yellow-700 rounded"
            >
              Continue
            </button>
            <button
              type="button"
              onClick={() => setShowWarning(false)}
              className="px-2 py-1 text-xs font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 rounded"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {mutation.isError && (
        <div className="text-sm text-red-600">
          Error: {mutation.error instanceof Error ? mutation.error.message : 'Execution failed'}
        </div>
      )}

      {result && !collapsed && (
        <div className="border border-gray-200 rounded-lg overflow-hidden">
          {/* Steps */}
          <div className="bg-gray-50 px-3 py-2 border-b border-gray-200">
            <h4 className="text-xs font-semibold text-gray-600 uppercase">Steps</h4>
          </div>
          <div className="divide-y divide-gray-100">
            {result.result.step_results.map((step, i) => (
              <div key={i} className="px-3 py-2 flex items-start gap-2 text-sm">
                <span
                  className={`mt-0.5 inline-block w-2 h-2 rounded-full flex-shrink-0 ${
                    step.success ? 'bg-green-500' : 'bg-red-500'
                  }`}
                />
                <div className="min-w-0">
                  <span className="font-medium text-gray-900">{step.step_name}</span>
                  {step.error_message && (
                    <p className="text-red-600 text-xs mt-0.5 break-words">
                      {step.error_message}
                    </p>
                  )}
                </div>
              </div>
            ))}
          </div>

          {/* Logs */}
          {result.logs.length > 0 && (
            <>
              <div className="bg-gray-50 px-3 py-2 border-t border-b border-gray-200">
                <h4 className="text-xs font-semibold text-gray-600 uppercase">Logs</h4>
              </div>
              <div className="divide-y divide-gray-100">
                {result.logs.map((log, i) => (
                  <div key={i} className="px-3 py-1.5 flex items-center gap-2 text-xs font-mono">
                    <span
                      className={`px-1.5 py-0.5 rounded text-xs font-medium ${
                        log.level === 'info'
                          ? 'bg-blue-100 text-blue-700'
                          : log.level === 'warn'
                            ? 'bg-yellow-100 text-yellow-700'
                            : 'bg-gray-100 text-gray-600'
                      }`}
                    >
                      {log.level}
                    </span>
                    <span className="text-gray-700">{log.message}</span>
                  </div>
                ))}
              </div>
            </>
          )}

          {/* Overall status */}
          <div
            className={`px-3 py-2 text-xs font-medium border-t ${
              result.result.success
                ? 'bg-green-50 text-green-700'
                : 'bg-red-50 text-red-700'
            }`}
          >
            {result.result.success ? 'Passed' : 'Failed'}
          </div>
        </div>
      )}
    </div>
  )
}
