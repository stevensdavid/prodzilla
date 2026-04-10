// --- Monitor configuration types ---

export type ExpectField = 'Body' | 'StatusCode'

export type ExpectOperation =
  | 'Equals'
  | 'NotEquals'
  | 'IsOneOf'
  | 'Contains'
  | 'NotContains'
  | 'Matches'

export interface Expectation {
  field: ExpectField
  operation: ExpectOperation
  value: string
}

export interface InputParameters {
  headers?: Record<string, string>
  body?: string
  timeout_seconds?: number
}

export interface Step {
  name: string
  url: string
  http_method: string
  with?: InputParameters
  expectations?: Expectation[]
  sensitive?: boolean
}

export interface ScheduleParameters {
  initial_delay: number
  interval: number
}

export interface Alert {
  url: string
}

export interface Monitor {
  name: string
  url?: string
  http_method?: string
  with?: InputParameters
  expectations?: Expectation[]
  sensitive?: boolean
  steps?: Step[]
  script?: string
  script_timeout_seconds?: number
  schedule: ScheduleParameters
  alerts?: Alert[]
  tags?: Record<string, string>
}

// --- API response types ---

export interface MonitorApiResponse {
  name: string
  monitor: Monitor
  version: number
  created_at: string
  updated_at: string
}

export interface MonitorListResponse {
  monitors: MonitorApiResponse[]
}

export interface UpdateMonitorRequest {
  monitor: Monitor
  version: number
}

export interface MonitorSummary {
  name: string
  status: string
  last_probed: string
}

// --- Result types ---

export interface EndpointResponse {
  timestamp_received: string
  status_code: number
  body: string
  sensitive: boolean
}

export interface StepResult {
  step_name: string
  timestamp_started: string
  success: boolean
  error_message?: string
  response?: EndpointResponse
  trace_id?: string
  span_id?: string
}

export interface MonitorResult {
  monitor_name: string
  timestamp_started: string
  success: boolean
  step_results: StepResult[]
}

// --- Error types ---

export interface ApiError {
  error: string
  message: string
  details?: Record<string, unknown>
}

export class ApiRequestError extends Error {
  status: number
  apiError?: ApiError

  constructor(status: number, apiError?: ApiError) {
    super(apiError?.message ?? `Request failed with status ${status}`)
    this.status = status
    this.apiError = apiError
  }
}

// --- Scripting API types ---

export interface ScriptingCompletionItem {
  label: string
  kind: string
  detail: string
  documentation?: string
  insert_text?: string
  insert_text_rules?: number
}

export interface CompletionsResponse {
  items: ScriptingCompletionItem[]
}

export interface ScriptDiagnostic {
  start_line: number
  start_column: number
  end_line: number
  end_column: number
  message: string
  severity: number
}

export interface ValidateScriptResponse {
  valid: boolean
  diagnostics: ScriptDiagnostic[]
}

export interface LogEntry {
  level: string
  message: string
  timestamp: string
}

export interface ExecuteScriptResponse {
  result: MonitorResult
  logs: LogEntry[]
}
