import type { Expectation, Monitor, Step, InputParameters } from '../types'
import type { StepFormState } from './step-form-utils'

// --- Form state shape ---

export type MonitorType = 'single' | 'multi' | 'scripted'

export interface MonitorFormState {
  name: string
  monitorType: MonitorType
  // Single-step fields
  url: string
  httpMethod: string
  headers: Array<{ key: string; value: string }>
  body: string
  timeoutSeconds: string
  sensitive: boolean
  expectations: Expectation[]
  // Multi-step fields
  steps: StepFormState[]
  // Scripted fields
  script: string
  scriptTimeoutSeconds: string
  // Common fields
  initialDelay: string
  interval: string
  alerts: Array<{ url: string }>
  tags: Array<{ key: string; value: string }>
}

export function emptyFormState(): MonitorFormState {
  return {
    name: '',
    monitorType: 'single',
    url: '',
    httpMethod: 'GET',
    headers: [],
    body: '',
    timeoutSeconds: '',
    sensitive: false,
    expectations: [],
    steps: [],
    script: '',
    scriptTimeoutSeconds: '30',
    initialDelay: '0',
    interval: '60',
    alerts: [],
    tags: [],
  }
}

export function monitorToFormState(monitor: Monitor): MonitorFormState {
  const common = {
    name: monitor.name,
    initialDelay: String(monitor.schedule.initial_delay),
    interval: String(monitor.schedule.interval),
    alerts: (monitor.alerts ?? []).map((a) => ({ url: a.url })),
    tags: Object.entries(monitor.tags ?? {}).map(([key, value]) => ({
      key,
      value,
    })),
  }

  if (monitor.script) {
    return {
      ...common,
      monitorType: 'scripted',
      url: '',
      httpMethod: 'GET',
      headers: [],
      body: '',
      timeoutSeconds: '',
      sensitive: false,
      expectations: [],
      steps: [],
      script: monitor.script,
      scriptTimeoutSeconds: String(monitor.script_timeout_seconds ?? 30),
    }
  }

  if (monitor.steps && monitor.steps.length > 0) {
    return {
      ...common,
      monitorType: 'multi',
      url: '',
      httpMethod: 'GET',
      headers: [],
      body: '',
      timeoutSeconds: '',
      sensitive: false,
      expectations: [],
      steps: monitor.steps.map(stepToFormState),
      script: '',
      scriptTimeoutSeconds: '30',
    }
  }

  return {
    ...common,
    monitorType: 'single',
    url: monitor.url ?? '',
    httpMethod: monitor.http_method ?? 'GET',
    headers: Object.entries(monitor.with?.headers ?? {}).map(([key, value]) => ({
      key,
      value,
    })),
    body: monitor.with?.body ?? '',
    timeoutSeconds: monitor.with?.timeout_seconds
      ? String(monitor.with.timeout_seconds)
      : '',
    sensitive: monitor.sensitive ?? false,
    expectations: monitor.expectations ?? [],
    steps: [],
    script: '',
    scriptTimeoutSeconds: '30',
  }
}

function stepToFormState(step: Step): StepFormState {
  return {
    name: step.name,
    url: step.url,
    httpMethod: step.http_method,
    headers: Object.entries(step.with?.headers ?? {}).map(([key, value]) => ({
      key,
      value,
    })),
    body: step.with?.body ?? '',
    timeoutSeconds: step.with?.timeout_seconds
      ? String(step.with.timeout_seconds)
      : '',
    sensitive: step.sensitive ?? false,
    expectations: step.expectations ?? [],
  }
}

function buildInputParameters(
  headers: Array<{ key: string; value: string }>,
  body: string,
  timeoutSeconds: string
): InputParameters | undefined {
  const headerMap: Record<string, string> = {}
  for (const h of headers) {
    if (h.key.trim()) headerMap[h.key.trim()] = h.value
  }
  const hasHeaders = Object.keys(headerMap).length > 0
  const hasBody = body.trim().length > 0
  const timeout = timeoutSeconds.trim() ? parseInt(timeoutSeconds, 10) : undefined

  if (!hasHeaders && !hasBody && timeout === undefined) return undefined

  return {
    headers: hasHeaders ? headerMap : undefined,
    body: hasBody ? body : undefined,
    timeout_seconds: timeout,
  }
}

export function formStateToMonitor(state: MonitorFormState): Monitor {
  const tags: Record<string, string> = {}
  for (const t of state.tags) {
    if (t.key.trim()) tags[t.key.trim()] = t.value
  }

  const base = {
    name: state.name.trim(),
    schedule: {
      initial_delay: parseInt(state.initialDelay, 10) || 0,
      interval: parseInt(state.interval, 10) || 60,
    },
    alerts:
      state.alerts.filter((a) => a.url.trim()).length > 0
        ? state.alerts.filter((a) => a.url.trim())
        : undefined,
    tags: Object.keys(tags).length > 0 ? tags : undefined,
  }

  if (state.monitorType === 'scripted') {
    return {
      ...base,
      script: state.script,
      script_timeout_seconds:
        parseInt(state.scriptTimeoutSeconds, 10) || 30,
    }
  }

  if (state.monitorType === 'multi') {
    const steps: Step[] = state.steps.map((s) => ({
      name: s.name.trim(),
      url: s.url.trim(),
      http_method: s.httpMethod,
      with: buildInputParameters(s.headers, s.body, s.timeoutSeconds),
      expectations: s.expectations.length > 0 ? s.expectations : undefined,
      sensitive: s.sensitive || undefined,
    }))
    return { ...base, steps }
  }

  return {
    ...base,
    url: state.url.trim(),
    http_method: state.httpMethod,
    with: buildInputParameters(state.headers, state.body, state.timeoutSeconds),
    expectations:
      state.expectations.length > 0 ? state.expectations : undefined,
    sensitive: state.sensitive || undefined,
  }
}

// --- Validation ---

export interface ValidationErrors {
  name?: string
  url?: string
  steps?: string
  script?: string
  interval?: string
  [key: string]: string | undefined
}

export function validateForm(state: MonitorFormState): ValidationErrors {
  const errors: ValidationErrors = {}

  if (!state.name.trim()) errors.name = 'Name is required'

  if (state.monitorType === 'single') {
    if (!state.url.trim()) errors.url = 'URL is required'
  } else if (state.monitorType === 'multi') {
    if (state.steps.length === 0) errors.steps = 'At least one step is required'
    for (let i = 0; i < state.steps.length; i++) {
      const s = state.steps[i]
      if (!s.name.trim()) errors[`step_${i}_name`] = 'Step name is required'
      if (!s.url.trim()) errors[`step_${i}_url`] = 'Step URL is required'
    }
    const names = state.steps.map((s) => s.name.trim()).filter(Boolean)
    if (new Set(names).size !== names.length) {
      errors.steps = 'Step names must be unique'
    }
  } else if (state.monitorType === 'scripted') {
    if (!state.script.trim()) errors.script = 'Script is required'
  }

  const interval = parseInt(state.interval, 10)
  if (isNaN(interval) || interval <= 0) {
    errors.interval = 'Interval must be greater than 0'
  }

  return errors
}
