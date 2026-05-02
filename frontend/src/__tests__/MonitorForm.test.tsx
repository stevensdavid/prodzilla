import { describe, it, expect, vi } from 'vitest'
import React from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import MonitorForm from '../components/MonitorForm'
import {
  formStateToMonitor,
  monitorToFormState,
  validateForm,
  emptyFormState,
} from '../components/monitor-form-utils'

vi.mock('../api')

vi.mock('../components/scripting/ScriptEditor', () => ({
  default: ({ value, onChange }: { value: string; onChange: (v: string) => void }) =>
    React.createElement('textarea', {
      'data-testid': 'script-editor',
      value: value ?? '',
      onChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => onChange?.(e.target.value),
    }),
}))

vi.mock('../components/scripting/DryRunPanel', () => ({
  default: ({ script }: { script: string }) =>
    React.createElement(
      'button',
      { type: 'button', disabled: !script.trim() },
      'Run Script'
    ),
}))

function renderForm(props: Partial<React.ComponentProps<typeof MonitorForm>> = {}) {
  const defaultProps = {
    onSubmit: vi.fn(),
    submitLabel: 'Save',
    isSubmitting: false,
    ...props,
  }
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return {
    ...render(
      <QueryClientProvider client={queryClient}>
        <MonitorForm {...defaultProps} />
      </QueryClientProvider>
    ),
    onSubmit: defaultProps.onSubmit,
  }
}

describe('MonitorForm', () => {
  it('renders in single-step mode by default', () => {
    renderForm()
    expect(screen.getByText('Request')).toBeInTheDocument()
    expect(screen.queryByText('Steps')).not.toBeInTheDocument()
  })

  it('toggles to multi-step mode', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getByLabelText('Multi-step'))
    expect(screen.getByText('Steps')).toBeInTheDocument()
    expect(screen.queryByText('Request')).not.toBeInTheDocument()
  })

  it('shows validation errors for missing required fields', async () => {
    const user = userEvent.setup()
    const { onSubmit } = renderForm()

    await user.click(screen.getByText('Save'))

    expect(screen.getByText('Name is required')).toBeInTheDocument()
    expect(screen.getByText('URL is required')).toBeInTheDocument()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('adds and removes expectations', async () => {
    const user = userEvent.setup()
    renderForm()

    // Initially no expectation fields
    expect(screen.queryByLabelText('Expectation field')).not.toBeInTheDocument()

    // Add expectation
    await user.click(screen.getAllByText('+ Add expectation')[0])
    expect(screen.getByLabelText('Expectation field')).toBeInTheDocument()

    // Remove it
    await user.click(screen.getByLabelText('Remove expectation'))
    expect(screen.queryByLabelText('Expectation field')).not.toBeInTheDocument()
  })

  it('adds and removes headers', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getAllByText('+ Add header')[0])
    const removeButtons = screen.getAllByLabelText('Remove header')
    expect(removeButtons).toHaveLength(1)

    await user.click(removeButtons[0])
    expect(screen.queryByLabelText('Remove header')).not.toBeInTheDocument()
  })

  it('adds and removes alerts', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getByText('+ Add alert'))
    expect(screen.getByLabelText('Remove alert')).toBeInTheDocument()

    await user.click(screen.getByLabelText('Remove alert'))
    expect(screen.queryByLabelText('Remove alert')).not.toBeInTheDocument()
  })

  it('adds and removes tags', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getByText('+ Add tag'))
    expect(screen.getByLabelText('Remove tag')).toBeInTheDocument()

    await user.click(screen.getByLabelText('Remove tag'))
    expect(screen.queryByLabelText('Remove tag')).not.toBeInTheDocument()
  })

  it('submits valid single-step monitor', async () => {
    const user = userEvent.setup()
    const { onSubmit } = renderForm()

    await user.type(screen.getByPlaceholderText('my-monitor'), 'test-mon')
    await user.type(screen.getByPlaceholderText('https://...'), 'https://example.com')

    await user.click(screen.getByText('Save'))

    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'test-mon',
        url: 'https://example.com',
        http_method: 'GET',
      })
    )
  })

  it('shows server error banner', () => {
    renderForm({ serverError: 'Monitor already exists' })
    expect(screen.getByText('Monitor already exists')).toBeInTheDocument()
  })

  it('disables name field when nameDisabled', () => {
    renderForm({ nameDisabled: true })
    expect(screen.getByPlaceholderText('my-monitor')).toBeDisabled()
  })

  it('pre-fills from initialState', () => {
    const initial = {
      ...emptyFormState(),
      name: 'existing',
      url: 'https://existing.com',
      interval: '30',
    }
    renderForm({ initialState: initial })

    expect(screen.getByPlaceholderText('my-monitor')).toHaveValue('existing')
    expect(screen.getByPlaceholderText('https://...')).toHaveValue('https://existing.com')
  })
})

describe('validateForm', () => {
  it('requires name', () => {
    const errors = validateForm(emptyFormState())
    expect(errors.name).toBe('Name is required')
  })

  it('requires URL for single-step', () => {
    const state = { ...emptyFormState(), name: 'test' }
    const errors = validateForm(state)
    expect(errors.url).toBe('URL is required')
  })

  it('requires at least one step for multi-step', () => {
    const state = { ...emptyFormState(), name: 'test', monitorType: 'multi' as const, steps: [] }
    const errors = validateForm(state)
    expect(errors.steps).toBe('At least one step is required')
  })

  it('requires positive interval', () => {
    const state = { ...emptyFormState(), name: 'test', url: 'http://x', interval: '0' }
    const errors = validateForm(state)
    expect(errors.interval).toBe('Interval must be greater than 0')
  })

  it('passes valid single-step form', () => {
    const state = { ...emptyFormState(), name: 'test', url: 'http://x' }
    const errors = validateForm(state)
    expect(Object.keys(errors)).toHaveLength(0)
  })
})

describe('formStateToMonitor', () => {
  it('converts single-step form to monitor', () => {
    const state = {
      ...emptyFormState(),
      name: 'test',
      url: 'https://example.com',
      httpMethod: 'POST',
      body: '{"key": "value"}',
      interval: '30',
      initialDelay: '5',
    }

    const monitor = formStateToMonitor(state)
    expect(monitor.name).toBe('test')
    expect(monitor.url).toBe('https://example.com')
    expect(monitor.http_method).toBe('POST')
    expect(monitor.with?.body).toBe('{"key": "value"}')
    expect(monitor.schedule).toEqual({ initial_delay: 5, interval: 30 })
    expect(monitor.steps).toBeUndefined()
  })

  it('omits empty optional fields', () => {
    const state = { ...emptyFormState(), name: 'test', url: 'http://x' }
    const monitor = formStateToMonitor(state)
    expect(monitor.with).toBeUndefined()
    expect(monitor.expectations).toBeUndefined()
    expect(monitor.alerts).toBeUndefined()
    expect(monitor.tags).toBeUndefined()
    expect(monitor.sensitive).toBeUndefined()
  })
})

describe('monitorToFormState', () => {
  it('converts single-step monitor to form state', () => {
    const state = monitorToFormState({
      name: 'test',
      url: 'https://example.com',
      http_method: 'GET',
      with: { headers: { 'x-api-key': 'abc' }, body: '{}', timeout_seconds: 10 },
      expectations: [{ field: 'StatusCode', operation: 'Equals', value: '200' }],
      schedule: { initial_delay: 0, interval: 60 },
      tags: { team: 'backend' },
    })

    expect(state.name).toBe('test')
    expect(state.monitorType).toBe('single')
    expect(state.url).toBe('https://example.com')
    expect(state.headers).toEqual([{ key: 'x-api-key', value: 'abc' }])
    expect(state.body).toBe('{}')
    expect(state.timeoutSeconds).toBe('10')
    expect(state.expectations).toHaveLength(1)
    expect(state.tags).toEqual([{ key: 'team', value: 'backend' }])
  })

  it('converts multi-step monitor to form state', () => {
    const state = monitorToFormState({
      name: 'test',
      steps: [
        { name: 'step-1', url: 'http://a', http_method: 'GET' },
        { name: 'step-2', url: 'http://b', http_method: 'POST' },
      ],
      schedule: { initial_delay: 0, interval: 60 },
    })

    expect(state.monitorType).toBe('multi')
    expect(state.steps).toHaveLength(2)
    expect(state.steps[0].name).toBe('step-1')
    expect(state.steps[1].name).toBe('step-2')
  })

  it('converts scripted monitor to form state', () => {
    const state = monitorToFormState({
      name: 'scripted-test',
      script: 'let x = 1;',
      script_timeout_seconds: 45,
      schedule: { initial_delay: 0, interval: 120 },
    })

    expect(state.monitorType).toBe('scripted')
    expect(state.script).toBe('let x = 1;')
    expect(state.scriptTimeoutSeconds).toBe('45')
  })

  it('defaults scriptTimeoutSeconds when not provided', () => {
    const state = monitorToFormState({
      name: 'scripted-test',
      script: 'let x = 1;',
      schedule: { initial_delay: 0, interval: 60 },
    })

    expect(state.scriptTimeoutSeconds).toBe('30')
  })
})

describe('formStateToMonitor (scripted)', () => {
  it('converts scripted form to monitor', () => {
    const state = {
      ...emptyFormState(),
      name: 'scripted-test',
      monitorType: 'scripted' as const,
      script: 'let x = 1;',
      scriptTimeoutSeconds: '45',
    }

    const monitor = formStateToMonitor(state)
    expect(monitor.name).toBe('scripted-test')
    expect(monitor.script).toBe('let x = 1;')
    expect(monitor.script_timeout_seconds).toBe(45)
  })

  it('scripted form omits single-step fields', () => {
    const state = {
      ...emptyFormState(),
      name: 'scripted-test',
      monitorType: 'scripted' as const,
      script: 'let x = 1;',
      scriptTimeoutSeconds: '30',
    }

    const monitor = formStateToMonitor(state)
    expect(monitor.url).toBeUndefined()
    expect(monitor.http_method).toBeUndefined()
    expect(monitor.steps).toBeUndefined()
  })
})

describe('validateForm (scripted)', () => {
  it('rejects empty script', () => {
    const state = {
      ...emptyFormState(),
      name: 'test',
      monitorType: 'scripted' as const,
      script: '',
    }
    const errors = validateForm(state)
    expect(errors.script).toBe('Script is required')
  })

  it('accepts non-empty script', () => {
    const state = {
      ...emptyFormState(),
      name: 'test',
      monitorType: 'scripted' as const,
      script: 'let x = 1;',
    }
    const errors = validateForm(state)
    expect(errors.script).toBeUndefined()
  })

  it('does not require url for scripted', () => {
    const state = {
      ...emptyFormState(),
      name: 'test',
      monitorType: 'scripted' as const,
      script: 'let x = 1;',
      url: '',
    }
    const errors = validateForm(state)
    expect(errors.url).toBeUndefined()
  })
})

describe('MonitorForm (scripted UI)', () => {
  it('renders three monitor type radio buttons', () => {
    renderForm()
    expect(screen.getByLabelText('Single-step')).toBeInTheDocument()
    expect(screen.getByLabelText('Multi-step')).toBeInTheDocument()
    expect(screen.getByLabelText('Scripted')).toBeInTheDocument()
  })

  it('selecting Scripted shows editor', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getByLabelText('Scripted'))
    expect(await screen.findByTestId('script-editor')).toBeInTheDocument()
  })

  it('selecting Scripted hides URL field', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getByLabelText('Scripted'))
    expect(screen.queryByPlaceholderText('https://...')).not.toBeInTheDocument()
  })

  it('selecting Scripted hides step fields', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getByLabelText('Scripted'))
    expect(screen.queryByText('Steps')).not.toBeInTheDocument()
  })

  it('switching from Scripted to Single-step shows URL', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getByLabelText('Scripted'))
    expect(screen.queryByPlaceholderText('https://...')).not.toBeInTheDocument()

    await user.click(screen.getByLabelText('Single-step'))
    expect(screen.getByPlaceholderText('https://...')).toBeInTheDocument()
  })

  it('scripted form validation shows error for empty script', async () => {
    const user = userEvent.setup()
    const { onSubmit } = renderForm()

    await user.type(screen.getByPlaceholderText('my-monitor'), 'test')
    await user.click(screen.getByLabelText('Scripted'))
    await user.click(screen.getByText('Save'))

    expect(screen.getByText('Script is required')).toBeInTheDocument()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it('initialState with scripted monitor pre-fills editor', () => {
    const initial = {
      ...emptyFormState(),
      name: 'existing-script',
      monitorType: 'scripted' as const,
      script: 'let code = 42;',
    }
    renderForm({ initialState: initial })

    expect(screen.getByTestId('script-editor')).toHaveValue('let code = 42;')
  })

  it('Run Script button appears in scripted mode', async () => {
    const user = userEvent.setup()
    renderForm()

    await user.click(screen.getByLabelText('Scripted'))
    expect(screen.getByText('Run Script')).toBeInTheDocument()
  })
})
