import { useState } from 'react'
import { useNavigate } from 'react-router'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { createMonitor } from '../api'
import type { Monitor } from '../types'
import { ApiRequestError } from '../types'
import MonitorForm from '../components/MonitorForm'

export default function MonitorCreate() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [serverError, setServerError] = useState<string>()

  const mutation = useMutation({
    mutationFn: (monitor: Monitor) => createMonitor(monitor),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['monitors'] })
      navigate(`/monitors/${encodeURIComponent(data.name)}`)
    },
    onError: (error) => {
      if (error instanceof ApiRequestError) {
        setServerError(error.apiError?.message ?? error.message)
      } else {
        setServerError(String(error))
      }
    },
  })

  return (
    <div className="max-w-2xl">
      <h1 className="text-xl font-semibold text-gray-900 mb-6">
        Create Monitor
      </h1>
      <MonitorForm
        onSubmit={(monitor) => mutation.mutate(monitor)}
        submitLabel="Create Monitor"
        isSubmitting={mutation.isPending}
        serverError={serverError}
      />
    </div>
  )
}
