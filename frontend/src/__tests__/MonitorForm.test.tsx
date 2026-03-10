import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import MonitorForm, {
  formStateToMonitor,
  monitorToFormState,
  validateForm,
  emptyFormState,
} from '../components/MonitorForm'

function renderForm(props: Partial<React.ComponentProps<typeof MonitorForm>> = {}) {
  const defaultProps = {
    onSubmit: vi.fn(),
    submitLabel: 'Save',
    isSubmitting: false,
    ...props,
  }
  return { ...render(<MonitorForm {...defaultProps} />), onSubmit: defaultProps.onSubmit }
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
    const state = { ...emptyFormState(), name: 'test', isMultiStep: true, steps: [] }
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
    expect(state.isMultiStep).toBe(false)
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

    expect(state.isMultiStep).toBe(true)
    expect(state.steps).toHaveLength(2)
    expect(state.steps[0].name).toBe('step-1')
    expect(state.steps[1].name).toBe('step-2')
  })
})
