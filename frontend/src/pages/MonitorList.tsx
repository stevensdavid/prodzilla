import { useState, useMemo } from 'react'
import { Link } from 'react-router'
import { useQuery } from '@tanstack/react-query'
import { listMonitors, listMonitorSummaries } from '../api'
import StatusBadge from '../components/StatusBadge'
import TagFilter from '../components/TagFilter'

export default function MonitorList() {
  const [activeTags, setActiveTags] = useState<Record<string, string>>({})

  const monitorsQuery = useQuery({
    queryKey: ['monitors'],
    queryFn: listMonitors,
    refetchInterval: 15000,
  })

  const summaryQuery = useQuery({
    queryKey: ['monitors-summary'],
    queryFn: listMonitorSummaries,
    refetchInterval: 15000,
  })

  const summaryMap = useMemo(() => {
    const map = new Map<string, { status: string; last_probed: string }>()
    for (const s of summaryQuery.data ?? []) {
      map.set(s.name, { status: s.status, last_probed: s.last_probed })
    }
    return map
  }, [summaryQuery.data])

  // Collect all tags
  const allTags = useMemo(() => {
    const tags: Record<string, Set<string>> = {}
    for (const m of monitorsQuery.data?.monitors ?? []) {
      for (const [key, value] of Object.entries(m.monitor.tags ?? {})) {
        if (!tags[key]) tags[key] = new Set()
        tags[key].add(value)
      }
    }
    return tags
  }, [monitorsQuery.data])

  // Filter monitors by active tags
  const filteredMonitors = useMemo(() => {
    const monitors = monitorsQuery.data?.monitors ?? []
    if (Object.keys(activeTags).length === 0) return monitors
    return monitors.filter((m) => {
      const tags = m.monitor.tags ?? {}
      return Object.entries(activeTags).every(
        ([key, value]) => tags[key] === value
      )
    })
  }, [monitorsQuery.data, activeTags])

  const handleTagToggle = (key: string, value: string) => {
    setActiveTags((prev) => {
      if (prev[key] === value) {
        const next = { ...prev }
        delete next[key]
        return next
      }
      return { ...prev, [key]: value }
    })
  }

  if (monitorsQuery.isLoading) {
    return <p className="text-sm text-gray-500">Loading monitors...</p>
  }

  if (monitorsQuery.isError) {
    return (
      <p className="text-sm text-red-600">
        Failed to load monitors: {String(monitorsQuery.error)}
      </p>
    )
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold text-gray-900">Monitors</h1>
        <Link
          to="/monitors/new"
          className="px-3 py-2 text-sm font-medium text-white bg-indigo-600 rounded-md hover:bg-indigo-700"
        >
          Create Monitor
        </Link>
      </div>

      <TagFilter
        allTags={allTags}
        activeTags={activeTags}
        onToggle={handleTagToggle}
        onClear={() => setActiveTags({})}
      />

      {filteredMonitors.length === 0 ? (
        <p className="text-sm text-gray-500">No monitors found.</p>
      ) : (
        <div className="overflow-hidden border border-gray-200 rounded-lg">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                  Name
                </th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                  Status
                </th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                  Type
                </th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                  Interval
                </th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                  Last Probed
                </th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                  Tags
                </th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              {filteredMonitors.map((m) => {
                const summary = summaryMap.get(m.name)
                return (
                  <tr key={m.name} className="hover:bg-gray-50">
                    <td className="px-4 py-3">
                      <Link
                        to={`/monitors/${encodeURIComponent(m.name)}`}
                        className="text-sm font-medium text-indigo-600 hover:text-indigo-800"
                      >
                        {m.name}
                      </Link>
                    </td>
                    <td className="px-4 py-3">
                      <StatusBadge status={summary?.status ?? 'PENDING'} />
                    </td>
                    <td className="px-4 py-3 text-sm text-gray-600">
                      {m.monitor.steps ? 'Multi-step' : 'Single-step'}
                    </td>
                    <td className="px-4 py-3 text-sm text-gray-600">
                      {m.monitor.schedule.interval}s
                    </td>
                    <td className="px-4 py-3 text-sm text-gray-500">
                      {summary?.last_probed
                        ? new Date(summary.last_probed).toLocaleString()
                        : '—'}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex flex-wrap gap-1">
                        {Object.entries(m.monitor.tags ?? {}).map(
                          ([k, v]) => (
                            <span
                              key={k}
                              className="inline-flex items-center rounded-full bg-gray-100 px-2 py-0.5 text-xs text-gray-600"
                            >
                              {k}:{v}
                            </span>
                          )
                        )}
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
