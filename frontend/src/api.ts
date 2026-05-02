import type {
  Monitor,
  MonitorApiResponse,
  MonitorListResponse,
  MonitorResult,
  MonitorSummary,
  CompletionsResponse,
  ValidateScriptResponse,
  ExecuteScriptResponse,
  ApiRequestError as ApiRequestErrorType,
} from './types'
import { ApiRequestError } from './types'

async function request<T>(url: string, options?: RequestInit): Promise<T> {
  const res = await fetch(url, options)
  if (!res.ok) {
    let apiError: ApiRequestErrorType['apiError']
    try {
      apiError = await res.json()
    } catch {
      // Response body wasn't JSON
    }
    throw new ApiRequestError(res.status, apiError)
  }
  if (res.status === 204) {
    return undefined as T
  }
  return res.json()
}

export async function listMonitors(): Promise<MonitorListResponse> {
  return request<MonitorListResponse>('/api/v1/monitors')
}

export async function listMonitorSummaries(): Promise<MonitorSummary[]> {
  return request<MonitorSummary[]>('/api/v1/monitors/summary')
}

export interface GetMonitorResponse {
  monitor: MonitorApiResponse
  version: number
}

export async function getMonitor(name: string): Promise<GetMonitorResponse> {
  const res = await fetch(`/api/v1/monitors/${encodeURIComponent(name)}`)
  if (!res.ok) {
    let apiError: ApiRequestErrorType['apiError']
    try {
      apiError = await res.json()
    } catch {
      // Response body wasn't JSON
    }
    throw new ApiRequestError(res.status, apiError)
  }
  const monitor: MonitorApiResponse = await res.json()
  const etag = res.headers.get('etag')
  const version = etag ? parseInt(etag, 10) : monitor.version
  return { monitor, version }
}

export async function createMonitor(
  monitor: Monitor
): Promise<MonitorApiResponse> {
  return request<MonitorApiResponse>('/api/v1/monitors', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(monitor),
  })
}

export async function updateMonitor(
  name: string,
  monitor: Monitor,
  version: number
): Promise<MonitorApiResponse> {
  return request<MonitorApiResponse>(
    `/api/v1/monitors/${encodeURIComponent(name)}`,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ monitor, version }),
    }
  )
}

export async function deleteMonitor(name: string): Promise<void> {
  return request<void>(`/api/v1/monitors/${encodeURIComponent(name)}`, {
    method: 'DELETE',
  })
}

export async function getMonitorResults(
  name: string,
  showResponse = false
): Promise<MonitorResult[]> {
  const params = showResponse ? '?show_response=true' : ''
  return request<MonitorResult[]>(
    `/api/v1/monitors/${encodeURIComponent(name)}/results${params}`
  )
}

export async function triggerMonitor(name: string): Promise<MonitorResult> {
  return request<MonitorResult>(
    `/api/v1/monitors/${encodeURIComponent(name)}/trigger`,
    { method: 'POST' }
  )
}

// --- Scripting endpoints ---

export async function getScriptingCompletions(): Promise<CompletionsResponse> {
  return request<CompletionsResponse>('/api/v1/scripting/completions')
}

export async function validateScript(
  script: string,
  signal?: AbortSignal
): Promise<ValidateScriptResponse> {
  return request<ValidateScriptResponse>('/api/v1/scripting/validate', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ script }),
    signal,
  })
}

export async function executeScript(
  script: string,
  timeoutSeconds?: number
): Promise<ExecuteScriptResponse> {
  return request<ExecuteScriptResponse>('/api/v1/scripting/execute', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      script,
      ...(timeoutSeconds !== undefined && {
        timeout_seconds: timeoutSeconds,
      }),
    }),
  })
}
