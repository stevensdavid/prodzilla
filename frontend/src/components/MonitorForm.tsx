import { lazy, Suspense, useState } from 'react'
import type { Monitor } from '../types'
import ExpectationForm from './ExpectationForm'
import {
  emptyFormState,
  formStateToMonitor,
  validateForm,
  type MonitorFormState,
  type ValidationErrors,
} from './monitor-form-utils'
import { emptyStepFormState } from './step-form-utils'
import StepFormComponent from './StepForm'

const ScriptEditor = lazy(() => import('./scripting/ScriptEditor'))
const DryRunPanel = lazy(() => import('./scripting/DryRunPanel'))

// --- Component ---

const HTTP_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']

interface MonitorFormProps {
  initialState?: MonitorFormState
  onSubmit: (monitor: Monitor) => void
  submitLabel: string
  isSubmitting: boolean
  serverError?: string
  nameDisabled?: boolean
}

export default function MonitorForm({
  initialState,
  onSubmit,
  submitLabel,
  isSubmitting,
  serverError,
  nameDisabled = false,
}: MonitorFormProps) {
  const [form, setForm] = useState<MonitorFormState>(
    initialState ?? emptyFormState()
  )
  const [errors, setErrors] = useState<ValidationErrors>({})

  const update = (partial: Partial<MonitorFormState>) =>
    setForm((prev) => ({ ...prev, ...partial }))

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const validationErrors = validateForm(form)
    setErrors(validationErrors)
    if (Object.keys(validationErrors).length > 0) return
    onSubmit(formStateToMonitor(form))
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {serverError && (
        <div className="rounded-md bg-red-50 border border-red-200 p-3">
          <p className="text-sm text-red-800">{serverError}</p>
        </div>
      )}

      {/* Name */}
      <div>
        <label className="block text-sm font-medium text-gray-700">Name</label>
        <input
          type="text"
          value={form.name}
          onChange={(e) => update({ name: e.target.value })}
          disabled={nameDisabled}
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm disabled:bg-gray-100"
          placeholder="my-monitor"
        />
        {errors.name && (
          <p className="mt-1 text-sm text-red-600">{errors.name}</p>
        )}
      </div>

      {/* Type toggle */}
      <div>
        <label className="block text-sm font-medium text-gray-700 mb-2">
          Type
        </label>
        <div className="flex gap-4">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="radio"
              checked={form.monitorType === 'single'}
              onChange={() => update({ monitorType: 'single' })}
              className="border-gray-300"
            />
            Single-step
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="radio"
              checked={form.monitorType === 'multi'}
              onChange={() =>
                update({
                  monitorType: 'multi',
                  steps:
                    form.steps.length > 0
                      ? form.steps
                      : [emptyStepFormState()],
                })
              }
              className="border-gray-300"
            />
            Multi-step
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="radio"
              checked={form.monitorType === 'scripted'}
              onChange={() => update({ monitorType: 'scripted' })}
              className="border-gray-300"
            />
            Scripted
          </label>
        </div>
      </div>

      {/* Single-step request fields */}
      {form.monitorType === 'single' && (
        <fieldset className="space-y-4 border border-gray-200 rounded-lg p-4">
          <legend className="text-sm font-semibold text-gray-700 px-1">
            Request
          </legend>

          <div className="grid grid-cols-[auto_1fr] gap-2">
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Method
              </label>
              <select
                value={form.httpMethod}
                onChange={(e) => update({ httpMethod: e.target.value })}
                className="mt-1 block rounded-md border border-gray-300 px-2 py-2 text-sm"
              >
                {HTTP_METHODS.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700">
                URL
              </label>
              <input
                type="text"
                value={form.url}
                onChange={(e) => update({ url: e.target.value })}
                className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm"
                placeholder="https://..."
              />
              {errors.url && (
                <p className="mt-1 text-sm text-red-600">{errors.url}</p>
              )}
            </div>
          </div>

          {/* Headers */}
          <div>
            <label className="block text-sm font-medium text-gray-700">
              Headers
            </label>
            <div className="space-y-1 mt-1">
              {form.headers.map((h, i) => (
                <div key={i} className="flex gap-2 items-center">
                  <input
                    type="text"
                    value={h.key}
                    onChange={(e) => {
                      const headers = [...form.headers]
                      headers[i] = { ...h, key: e.target.value }
                      update({ headers })
                    }}
                    placeholder="key"
                    className="block w-40 rounded-md border border-gray-300 px-2 py-1.5 text-sm"
                  />
                  <span className="text-gray-400">:</span>
                  <input
                    type="text"
                    value={h.value}
                    onChange={(e) => {
                      const headers = [...form.headers]
                      headers[i] = { ...h, value: e.target.value }
                      update({ headers })
                    }}
                    placeholder="value"
                    className="block flex-1 rounded-md border border-gray-300 px-2 py-1.5 text-sm"
                  />
                  <button
                    type="button"
                    onClick={() =>
                      update({
                        headers: form.headers.filter((_, j) => j !== i),
                      })
                    }
                    className="text-gray-400 hover:text-red-500"
                    aria-label="Remove header"
                  >
                    &times;
                  </button>
                </div>
              ))}
            </div>
            <button
              type="button"
              onClick={() =>
                update({
                  headers: [...form.headers, { key: '', value: '' }],
                })
              }
              className="mt-1 text-xs text-indigo-600 hover:text-indigo-800"
            >
              + Add header
            </button>
          </div>

          {/* Body */}
          <div>
            <label className="block text-sm font-medium text-gray-700">
              Body
            </label>
            <textarea
              value={form.body}
              onChange={(e) => update({ body: e.target.value })}
              rows={3}
              className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm font-mono"
              placeholder="Request body (optional)"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-gray-700">
                Timeout (s)
              </label>
              <input
                type="number"
                value={form.timeoutSeconds}
                onChange={(e) => update({ timeoutSeconds: e.target.value })}
                className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm"
                placeholder="Optional"
              />
            </div>
            <div className="flex items-end">
              <label className="flex items-center gap-2 text-sm text-gray-700">
                <input
                  type="checkbox"
                  checked={form.sensitive}
                  onChange={(e) => update({ sensitive: e.target.checked })}
                  className="rounded border-gray-300"
                />
                Sensitive
              </label>
            </div>
          </div>

          {/* Expectations */}
          <div>
            <label className="block text-sm font-medium text-gray-700">
              Expectations
            </label>
            <div className="space-y-2 mt-1">
              {form.expectations.map((exp, i) => (
                <ExpectationForm
                  key={i}
                  value={exp}
                  onChange={(updated) => {
                    const expectations = [...form.expectations]
                    expectations[i] = updated
                    update({ expectations })
                  }}
                  onRemove={() =>
                    update({
                      expectations: form.expectations.filter(
                        (_, j) => j !== i
                      ),
                    })
                  }
                />
              ))}
            </div>
            <button
              type="button"
              onClick={() =>
                update({
                  expectations: [
                    ...form.expectations,
                    { field: 'StatusCode', operation: 'Equals', value: '' },
                  ],
                })
              }
              className="mt-1 text-xs text-indigo-600 hover:text-indigo-800"
            >
              + Add expectation
            </button>
          </div>
        </fieldset>
      )}

      {/* Scripted */}
      {form.monitorType === 'scripted' && (
        <fieldset className="space-y-4 border border-gray-200 rounded-lg p-4">
          <legend className="text-sm font-semibold text-gray-700 px-1">
            Script
          </legend>
          {errors.script && (
            <p className="text-sm text-red-600">{errors.script}</p>
          )}
          <Suspense
            fallback={
              <div className="h-[400px] bg-gray-900 rounded-md flex items-center justify-center text-gray-400 text-sm">
                Loading editor...
              </div>
            }
          >
            <ScriptEditor
              value={form.script}
              onChange={(script) => update({ script })}
            />
          </Suspense>
          <div className="w-48">
            <label className="block text-sm font-medium text-gray-700">
              Timeout (s)
            </label>
            <input
              type="number"
              value={form.scriptTimeoutSeconds}
              onChange={(e) =>
                update({ scriptTimeoutSeconds: e.target.value })
              }
              className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm"
              min="1"
              max="120"
            />
          </div>
          <Suspense fallback={null}>
            <DryRunPanel
              script={form.script}
              timeoutSeconds={
                parseInt(form.scriptTimeoutSeconds, 10) || 30
              }
            />
          </Suspense>
        </fieldset>
      )}

      {/* Multi-step */}
      {form.monitorType === 'multi' && (
        <fieldset className="space-y-4 border border-gray-200 rounded-lg p-4">
          <legend className="text-sm font-semibold text-gray-700 px-1">
            Steps
          </legend>
          {errors.steps && (
            <p className="text-sm text-red-600">{errors.steps}</p>
          )}
          {form.steps.map((step, i) => (
            <StepFormComponent
              key={i}
              index={i}
              value={step}
              onChange={(updated) => {
                const steps = [...form.steps]
                steps[i] = updated
                update({ steps })
              }}
              onRemove={() =>
                update({ steps: form.steps.filter((_, j) => j !== i) })
              }
              previousStepNames={form.steps
                .slice(0, i)
                .map((s) => s.name)
                .filter(Boolean)}
            />
          ))}
          <button
            type="button"
            onClick={() =>
              update({ steps: [...form.steps, emptyStepFormState()] })
            }
            className="text-sm text-indigo-600 hover:text-indigo-800"
          >
            + Add Step
          </button>
        </fieldset>
      )}

      {/* Schedule */}
      <fieldset className="space-y-3 border border-gray-200 rounded-lg p-4">
        <legend className="text-sm font-semibold text-gray-700 px-1">
          Schedule
        </legend>
        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700">
              Initial Delay (s)
            </label>
            <input
              type="number"
              value={form.initialDelay}
              onChange={(e) => update({ initialDelay: e.target.value })}
              className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700">
              Interval (s)
            </label>
            <input
              type="number"
              value={form.interval}
              onChange={(e) => update({ interval: e.target.value })}
              className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm"
            />
            {errors.interval && (
              <p className="mt-1 text-sm text-red-600">{errors.interval}</p>
            )}
          </div>
        </div>
      </fieldset>

      {/* Alerts */}
      <fieldset className="space-y-3 border border-gray-200 rounded-lg p-4">
        <legend className="text-sm font-semibold text-gray-700 px-1">
          Alerts
        </legend>
        {form.alerts.map((alert, i) => (
          <div key={i} className="flex gap-2 items-center">
            <input
              type="text"
              value={alert.url}
              onChange={(e) => {
                const alerts = [...form.alerts]
                alerts[i] = { url: e.target.value }
                update({ alerts })
              }}
              placeholder="https://webhook.example.com/..."
              className="block flex-1 rounded-md border border-gray-300 px-3 py-1.5 text-sm"
            />
            <button
              type="button"
              onClick={() =>
                update({ alerts: form.alerts.filter((_, j) => j !== i) })
              }
              className="text-gray-400 hover:text-red-500"
              aria-label="Remove alert"
            >
              &times;
            </button>
          </div>
        ))}
        <button
          type="button"
          onClick={() => update({ alerts: [...form.alerts, { url: '' }] })}
          className="text-xs text-indigo-600 hover:text-indigo-800"
        >
          + Add alert
        </button>
      </fieldset>

      {/* Tags */}
      <fieldset className="space-y-3 border border-gray-200 rounded-lg p-4">
        <legend className="text-sm font-semibold text-gray-700 px-1">
          Tags
        </legend>
        {form.tags.map((tag, i) => (
          <div key={i} className="flex gap-2 items-center">
            <input
              type="text"
              value={tag.key}
              onChange={(e) => {
                const tags = [...form.tags]
                tags[i] = { ...tag, key: e.target.value }
                update({ tags })
              }}
              placeholder="key"
              className="block w-40 rounded-md border border-gray-300 px-2 py-1.5 text-sm"
            />
            <span className="text-gray-400">:</span>
            <input
              type="text"
              value={tag.value}
              onChange={(e) => {
                const tags = [...form.tags]
                tags[i] = { ...tag, value: e.target.value }
                update({ tags })
              }}
              placeholder="value"
              className="block flex-1 rounded-md border border-gray-300 px-2 py-1.5 text-sm"
            />
            <button
              type="button"
              onClick={() =>
                update({ tags: form.tags.filter((_, j) => j !== i) })
              }
              className="text-gray-400 hover:text-red-500"
              aria-label="Remove tag"
            >
              &times;
            </button>
          </div>
        ))}
        <button
          type="button"
          onClick={() =>
            update({ tags: [...form.tags, { key: '', value: '' }] })
          }
          className="text-xs text-indigo-600 hover:text-indigo-800"
        >
          + Add tag
        </button>
      </fieldset>

      {/* Submit */}
      <div className="flex justify-end gap-3">
        <button
          type="submit"
          disabled={isSubmitting}
          className="px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-md hover:bg-indigo-700 disabled:opacity-50"
        >
          {isSubmitting ? 'Saving...' : submitLabel}
        </button>
      </div>
    </form>
  )
}
