import { useState } from 'react'
import { useParams, useNavigate, Link } from 'react-router'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  getMonitor,
  getMonitorResults,
  triggerMonitor,
  deleteMonitor,
} from '../api'
import StatusBadge from '../components/StatusBadge'
import ResultTimeline from '../components/ResultTimeline'
import ConfirmDialog from '../components/ConfirmDialog'

export default function MonitorDetail() {
  const { name } = useParams<{ name: string }>()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)

  const monitorQuery = useQuery({
    queryKey: ['monitor', name],
    queryFn: () => getMonitor(name!),
    enabled: !!name,
  })

  const resultsQuery = useQuery({
    queryKey: ['monitor-results', name],
    queryFn: () => getMonitorResults(name!, true),
    enabled: !!name,
    refetchInterval: 15000,
  })

  const triggerMutation = useMutation({
    mutationFn: () => triggerMonitor(name!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['monitor-results', name] })
      queryClient.invalidateQueries({ queryKey: ['monitors-summary'] })
    },
  })

  const deleteMutation = useMutation({
    mutationFn: () => deleteMonitor(name!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['monitors'] })
      navigate('/')
    },
  })

  if (monitorQuery.isLoading) {
    return <p className="text-sm text-gray-500">Loading...</p>
  }

  if (monitorQuery.isError) {
    return (
      <p className="text-sm text-red-600">
        Failed to load monitor: {String(monitorQuery.error)}
      </p>
    )
  }

  const { monitor: monitorResponse } = monitorQuery.data!
  const monitor = monitorResponse.monitor
  const results = resultsQuery.data ?? []
  const latestResult = results[0]
  const status = latestResult
    ? latestResult.success
      ? 'OK'
      : 'FAILING'
    : 'PENDING'

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-xl font-semibold text-gray-900">
            {monitor.name}
          </h1>
          <StatusBadge status={status} />
        </div>
        <div className="flex gap-2">
          <button
            onClick={() => triggerMutation.mutate()}
            disabled={triggerMutation.isPending}
            className="px-3 py-2 text-sm font-medium text-white bg-green-600 rounded-md hover:bg-green-700 disabled:opacity-50"
          >
            {triggerMutation.isPending ? 'Running...' : 'Trigger Now'}
          </button>
          <Link
            to={`/monitors/${encodeURIComponent(name!)}/edit`}
            className="px-3 py-2 text-sm font-medium text-gray-700 bg-white border border-gray-300 rounded-md hover:bg-gray-50"
          >
            Edit
          </Link>
          <button
            onClick={() => setShowDeleteConfirm(true)}
            className="px-3 py-2 text-sm font-medium text-red-600 bg-white border border-red-300 rounded-md hover:bg-red-50"
          >
            Delete
          </button>
        </div>
      </div>

      {/* Config summary */}
      <div className="border border-gray-200 rounded-lg p-4 space-y-3">
        <h2 className="text-sm font-semibold text-gray-700">Configuration</h2>

        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span className="text-gray-500">Type: </span>
            <span className="text-gray-900">
              {monitor.script
                ? 'Scripted'
                : monitor.steps
                  ? 'Multi-step'
                  : 'Single-step'}
            </span>
          </div>
          <div>
            <span className="text-gray-500">Schedule: </span>
            <span className="text-gray-900">
              every {monitor.schedule.interval}s (delay:{' '}
              {monitor.schedule.initial_delay}s)
            </span>
          </div>
          {monitor.url && (
            <div className="col-span-2">
              <span className="text-gray-500">
                {monitor.http_method} {monitor.url}
              </span>
            </div>
          )}
        </div>

        {/* Script for scripted */}
        {monitor.script && (
          <div>
            <h3 className="text-sm font-medium text-gray-600 mb-1">Script</h3>
            <pre className="text-xs text-gray-700 bg-gray-900 text-gray-100 rounded px-3 py-2 overflow-x-auto max-h-60 overflow-y-auto">
              {monitor.script}
            </pre>
            {monitor.script_timeout_seconds && (
              <div className="text-sm text-gray-500 mt-1">
                Timeout: {monitor.script_timeout_seconds}s
              </div>
            )}
          </div>
        )}

        {/* Steps for multi-step */}
        {monitor.steps && (
          <div>
            <h3 className="text-sm font-medium text-gray-600 mb-1">Steps</h3>
            <div className="space-y-1">
              {monitor.steps.map((step, i) => (
                <div
                  key={i}
                  className="text-sm text-gray-600 bg-gray-50 rounded px-3 py-1.5"
                >
                  <span className="font-medium">{step.name}:</span>{' '}
                  {step.http_method} {step.url}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Expectations */}
        {monitor.expectations && monitor.expectations.length > 0 && (
          <div>
            <h3 className="text-sm font-medium text-gray-600 mb-1">
              Expectations
            </h3>
            <div className="space-y-1">
              {monitor.expectations.map((exp, i) => (
                <div key={i} className="text-sm text-gray-600">
                  {exp.field} {exp.operation} &quot;{exp.value}&quot;
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Alerts */}
        {monitor.alerts && monitor.alerts.length > 0 && (
          <div>
            <h3 className="text-sm font-medium text-gray-600 mb-1">Alerts</h3>
            {monitor.alerts.map((alert, i) => (
              <div key={i} className="text-sm text-gray-600">
                {alert.url}
              </div>
            ))}
          </div>
        )}

        {/* Tags */}
        {monitor.tags && Object.keys(monitor.tags).length > 0 && (
          <div className="flex flex-wrap gap-1">
            {Object.entries(monitor.tags).map(([k, v]) => (
              <span
                key={k}
                className="inline-flex items-center rounded-full bg-gray-100 px-2.5 py-0.5 text-xs text-gray-600"
              >
                {k}:{v}
              </span>
            ))}
          </div>
        )}

        <div className="text-xs text-gray-400">
          Version {monitorResponse.version} &middot; Created{' '}
          {new Date(monitorResponse.created_at).toLocaleString()} &middot;
          Updated {new Date(monitorResponse.updated_at).toLocaleString()}
        </div>
      </div>

      {/* Results */}
      <div>
        <h2 className="text-sm font-semibold text-gray-700 mb-2">
          Recent Results
        </h2>
        {resultsQuery.isLoading ? (
          <p className="text-sm text-gray-500">Loading results...</p>
        ) : (
          <ResultTimeline results={results} />
        )}
      </div>

      {/* Delete confirmation */}
      {showDeleteConfirm && (
        <ConfirmDialog
          title="Delete Monitor"
          message={`Are you sure you want to delete "${monitor.name}"? This cannot be undone.`}
          confirmLabel="Delete"
          onConfirm={() => deleteMutation.mutate()}
          onCancel={() => setShowDeleteConfirm(false)}
        />
      )}
    </div>
  )
}
