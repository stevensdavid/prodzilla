import { useState } from 'react'
import type { MonitorResult } from '../types'

interface ResultTimelineProps {
  results: MonitorResult[]
}

export default function ResultTimeline({ results }: ResultTimelineProps) {
  if (results.length === 0) {
    return <p className="text-sm text-gray-500">No results yet.</p>
  }

  return (
    <div className="space-y-2">
      {results.map((result, i) => (
        <ResultRow key={i} result={result} />
      ))}
    </div>
  )
}

function ResultRow({ result }: { result: MonitorResult }) {
  const [expanded, setExpanded] = useState(false)
  const time = new Date(result.timestamp_started)

  return (
    <div className="border border-gray-200 rounded-md">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-4 py-2 text-left hover:bg-gray-50"
      >
        <div className="flex items-center gap-3">
          <span
            className={`w-2 h-2 rounded-full ${result.success ? 'bg-green-500' : 'bg-red-500'}`}
          />
          <span className="text-sm font-medium text-gray-900">
            {result.success ? 'Pass' : 'Fail'}
          </span>
          <span className="text-sm text-gray-500">
            {result.step_results.length} step
            {result.step_results.length !== 1 ? 's' : ''}
          </span>
        </div>
        <span className="text-xs text-gray-400">{time.toLocaleString()}</span>
      </button>
      {expanded && (
        <div className="border-t border-gray-200 px-4 py-3 space-y-3">
          {result.step_results.map((step, i) => (
            <div key={i} className="text-sm">
              <div className="flex items-center gap-2">
                <span
                  className={`w-1.5 h-1.5 rounded-full ${step.success ? 'bg-green-500' : 'bg-red-500'}`}
                />
                <span className="font-medium text-gray-800">
                  {step.step_name}
                </span>
                {step.response && (
                  <span className="text-gray-500">
                    {step.response.status_code}
                  </span>
                )}
              </div>
              {step.error_message && (
                <p className="mt-1 text-red-600 text-xs ml-3.5">
                  {step.error_message}
                </p>
              )}
              {step.response && !step.response.sensitive && (
                <pre className="mt-1 ml-3.5 text-xs text-gray-600 bg-gray-50 rounded p-2 overflow-x-auto max-h-40">
                  {step.response.body}
                </pre>
              )}
              {step.response?.sensitive && (
                <p className="mt-1 ml-3.5 text-xs text-gray-400 italic">
                  Response hidden (sensitive)
                </p>
              )}
              {(step.trace_id || step.span_id) && (
                <div className="mt-1 ml-3.5 text-xs text-gray-400">
                  {step.trace_id && <span>trace: {step.trace_id} </span>}
                  {step.span_id && <span>span: {step.span_id}</span>}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
