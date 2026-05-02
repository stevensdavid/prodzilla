import type { Expectation, ExpectField, ExpectOperation } from '../types'

interface ExpectationFormProps {
  value: Expectation
  onChange: (updated: Expectation) => void
  onRemove: () => void
}

const FIELDS: ExpectField[] = ['StatusCode', 'Body']
const OPERATIONS: ExpectOperation[] = [
  'Equals',
  'NotEquals',
  'Contains',
  'NotContains',
  'IsOneOf',
  'Matches',
]

function getPlaceholder(operation: ExpectOperation): string {
  switch (operation) {
    case 'IsOneOf':
      return '200|201|204'
    case 'Matches':
      return '^\\d{3}$'
    default:
      return ''
  }
}

function getHint(operation: ExpectOperation): string | null {
  switch (operation) {
    case 'IsOneOf':
      return 'Separate values with |'
    case 'Matches':
      return 'Regular expression'
    default:
      return null
  }
}

export default function ExpectationForm({
  value,
  onChange,
  onRemove,
}: ExpectationFormProps) {
  const hint = getHint(value.operation)

  return (
    <div className="flex items-start gap-2">
      <select
        value={value.field}
        onChange={(e) => onChange({ ...value, field: e.target.value as ExpectField })}
        className="block w-32 rounded-md border border-gray-300 px-2 py-1.5 text-sm"
        aria-label="Expectation field"
      >
        {FIELDS.map((f) => (
          <option key={f} value={f}>
            {f}
          </option>
        ))}
      </select>
      <select
        value={value.operation}
        onChange={(e) =>
          onChange({ ...value, operation: e.target.value as ExpectOperation })
        }
        className="block w-36 rounded-md border border-gray-300 px-2 py-1.5 text-sm"
        aria-label="Expectation operation"
      >
        {OPERATIONS.map((op) => (
          <option key={op} value={op}>
            {op}
          </option>
        ))}
      </select>
      <div className="flex-1">
        <input
          type="text"
          value={value.value}
          onChange={(e) => onChange({ ...value, value: e.target.value })}
          placeholder={getPlaceholder(value.operation)}
          className="block w-full rounded-md border border-gray-300 px-2 py-1.5 text-sm"
          aria-label="Expectation value"
        />
        {hint && <p className="mt-0.5 text-xs text-gray-400">{hint}</p>}
      </div>
      <button
        type="button"
        onClick={onRemove}
        className="text-gray-400 hover:text-red-500 p-1"
        aria-label="Remove expectation"
      >
        &times;
      </button>
    </div>
  )
}
