interface StatusBadgeProps {
  status: string
}

export default function StatusBadge({ status }: StatusBadgeProps) {
  const colorClasses =
    status === 'OK'
      ? 'bg-green-100 text-green-800'
      : status === 'FAILING'
        ? 'bg-red-100 text-red-800'
        : 'bg-gray-100 text-gray-800'

  return (
    <span
      className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${colorClasses}`}
    >
      {status}
    </span>
  )
}
