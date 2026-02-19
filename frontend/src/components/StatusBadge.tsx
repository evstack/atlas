interface StatusBadgeProps {
  status: boolean;
}

export default function StatusBadge({ status }: StatusBadgeProps) {
  return (
    <span
      className={`inline-flex items-center px-2.5 py-0.5 rounded-full border text-xs font-semibold leading-none bg-dark-600 border-dark-500 ${
        status ? 'text-accent-success' : 'text-accent-error'
      }`}
      title={status ? 'Transaction succeeded' : 'Transaction failed'}
    >
      {status ? 'Success' : 'Failed'}
    </span>
  );
}
