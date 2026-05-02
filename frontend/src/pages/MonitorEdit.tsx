import { useState } from 'react'
import { useParams, useNavigate } from 'react-router'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { getMonitor, updateMonitor } from '../api'
import type { Monitor } from '../types'
import { ApiRequestError } from '../types'
import MonitorForm from '../components/MonitorForm'
import { monitorToFormState } from '../components/monitor-form-utils'

export default function MonitorEdit() {
  const { name } = useParams<{ name: string }>()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [serverError, setServerError] = useState<string>()

  const monitorQuery = useQuery({
    queryKey: ['monitor', name],
    queryFn: () => getMonitor(name!),
    enabled: !!name,
  })

  const mutation = useMutation({
    mutationFn: (monitor: Monitor) =>
      updateMonitor(name!, monitor, monitorQuery.data!.version),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['monitor', name] })
      queryClient.invalidateQueries({ queryKey: ['monitors'] })
      navigate(`/monitors/${encodeURIComponent(name!)}`)
    },
    onError: (error) => {
      if (error instanceof ApiRequestError) {
        if (error.status === 409) {
          setServerError(
            'This monitor was modified by another user. Please refresh and try again.'
          )
        } else {
          setServerError(error.apiError?.message ?? error.message)
        }
      } else {
        setServerError(String(error))
      }
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
  const initialState = monitorToFormState(monitorResponse.monitor)

  return (
    <div className="max-w-2xl">
      <h1 className="text-xl font-semibold text-gray-900 mb-6">
        Edit Monitor: {name}
      </h1>
      <MonitorForm
        initialState={initialState}
        onSubmit={(monitor) => mutation.mutate(monitor)}
        submitLabel="Save Changes"
        isSubmitting={mutation.isPending}
        serverError={serverError}
        nameDisabled
      />
    </div>
  )
}
