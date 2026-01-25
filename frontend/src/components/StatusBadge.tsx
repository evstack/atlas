interface StatusBadgeProps {
  status: boolean;
}

export default function StatusBadge({ status }: StatusBadgeProps) {
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 text-xs font-medium ${
        status
          ? 'text-accent-success'
          : 'text-accent-error'
      }`}
    >
      {status ? 'Success' : 'Failed'}
    </span>
  );
}
