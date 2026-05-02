interface TagFilterProps {
  allTags: Record<string, Set<string>>
  activeTags: Record<string, string>
  onToggle: (key: string, value: string) => void
  onClear: () => void
}

export default function TagFilter({
  allTags,
  activeTags,
  onToggle,
  onClear,
}: TagFilterProps) {
  const tagEntries = Object.entries(allTags)
  if (tagEntries.length === 0) return null

  const hasActive = Object.keys(activeTags).length > 0

  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-xs font-medium text-gray-500 uppercase">
        Filter:
      </span>
      {tagEntries.map(([key, values]) =>
        [...values].sort().map((value) => {
          const isActive = activeTags[key] === value
          return (
            <button
              key={`${key}:${value}`}
              onClick={() => onToggle(key, value)}
              className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors ${
                isActive
                  ? 'bg-indigo-100 text-indigo-800 ring-1 ring-indigo-300'
                  : 'bg-gray-100 text-gray-600 hover:bg-gray-200'
              }`}
            >
              {key}:{value}
            </button>
          )
        })
      )}
      {hasActive && (
        <button
          onClick={onClear}
          className="text-xs text-gray-500 hover:text-gray-700 underline"
        >
          Clear
        </button>
      )}
    </div>
  )
}
