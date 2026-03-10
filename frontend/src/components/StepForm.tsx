import type { Expectation } from '../types'
import ExpectationForm from './ExpectationForm'

export interface StepFormState {
  name: string
  url: string
  httpMethod: string
  headers: Array<{ key: string; value: string }>
  body: string
  timeoutSeconds: string
  sensitive: boolean
  expectations: Expectation[]
}

interface StepFormProps {
  index: number
  value: StepFormState
  onChange: (updated: StepFormState) => void
  onRemove: () => void
  previousStepNames: string[]
}

const HTTP_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']

export function emptyStepFormState(): StepFormState {
  return {
    name: '',
    url: '',
    httpMethod: 'GET',
    headers: [],
    body: '',
    timeoutSeconds: '',
    sensitive: false,
    expectations: [],
  }
}

export default function StepForm({
  index,
  value,
  onChange,
  onRemove,
  previousStepNames,
}: StepFormProps) {
  const update = (partial: Partial<StepFormState>) =>
    onChange({ ...value, ...partial })

  return (
    <div className="border border-gray-200 rounded-lg p-4 space-y-3">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-gray-700">
          Step {index + 1}
        </h4>
        <button
          type="button"
          onClick={onRemove}
          className="text-gray-400 hover:text-red-500 text-sm"
          aria-label={`Remove step ${index + 1}`}
        >
          &times; Remove
        </button>
      </div>

      <div>
        <label className="block text-sm font-medium text-gray-700">Name</label>
        <input
          type="text"
          value={value.name}
          onChange={(e) => update({ name: e.target.value })}
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-1.5 text-sm"
          placeholder="step-name"
        />
      </div>

      <div className="grid grid-cols-[auto_1fr] gap-2">
        <div>
          <label className="block text-sm font-medium text-gray-700">
            Method
          </label>
          <select
            value={value.httpMethod}
            onChange={(e) => update({ httpMethod: e.target.value })}
            className="mt-1 block rounded-md border border-gray-300 px-2 py-1.5 text-sm"
          >
            {HTTP_METHODS.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700">URL</label>
          <input
            type="text"
            value={value.url}
            onChange={(e) => update({ url: e.target.value })}
            className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-1.5 text-sm"
            placeholder="https://..."
          />
        </div>
      </div>

      {/* Headers */}
      <div>
        <label className="block text-sm font-medium text-gray-700">
          Headers
        </label>
        <div className="space-y-1 mt-1">
          {value.headers.map((h, i) => (
            <div key={i} className="flex gap-2 items-center">
              <input
                type="text"
                value={h.key}
                onChange={(e) => {
                  const headers = [...value.headers]
                  headers[i] = { ...h, key: e.target.value }
                  update({ headers })
                }}
                placeholder="key"
                className="block w-40 rounded-md border border-gray-300 px-2 py-1 text-sm"
              />
              <span className="text-gray-400">:</span>
              <input
                type="text"
                value={h.value}
                onChange={(e) => {
                  const headers = [...value.headers]
                  headers[i] = { ...h, value: e.target.value }
                  update({ headers })
                }}
                placeholder="value"
                className="block flex-1 rounded-md border border-gray-300 px-2 py-1 text-sm"
              />
              <button
                type="button"
                onClick={() =>
                  update({ headers: value.headers.filter((_, j) => j !== i) })
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
            update({ headers: [...value.headers, { key: '', value: '' }] })
          }
          className="mt-1 text-xs text-indigo-600 hover:text-indigo-800"
        >
          + Add header
        </button>
      </div>

      {/* Body */}
      <div>
        <label className="block text-sm font-medium text-gray-700">Body</label>
        <textarea
          value={value.body}
          onChange={(e) => update({ body: e.target.value })}
          rows={2}
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-1.5 text-sm font-mono"
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
            value={value.timeoutSeconds}
            onChange={(e) => update({ timeoutSeconds: e.target.value })}
            className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-1.5 text-sm"
            placeholder="Optional"
          />
        </div>
        <div className="flex items-end">
          <label className="flex items-center gap-2 text-sm text-gray-700">
            <input
              type="checkbox"
              checked={value.sensitive}
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
          {value.expectations.map((exp, i) => (
            <ExpectationForm
              key={i}
              value={exp}
              onChange={(updated) => {
                const expectations = [...value.expectations]
                expectations[i] = updated
                update({ expectations })
              }}
              onRemove={() =>
                update({
                  expectations: value.expectations.filter((_, j) => j !== i),
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
                ...value.expectations,
                { field: 'StatusCode', operation: 'Equals', value: '' },
              ],
            })
          }
          className="mt-1 text-xs text-indigo-600 hover:text-indigo-800"
        >
          + Add expectation
        </button>
      </div>

      {/* Variable hint */}
      {previousStepNames.length > 0 && (
        <p className="text-xs text-gray-400">
          Tip: Reference previous step outputs with{' '}
          <code className="bg-gray-100 px-1 rounded">
            {'${{ steps.'}
            {previousStepNames[previousStepNames.length - 1]}
            {'.response.body.field }}'}
          </code>
        </p>
      )}
    </div>
  )
}
