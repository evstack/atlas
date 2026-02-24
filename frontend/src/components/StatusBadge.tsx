interface StatusBadgeProps {
  status: boolean;
}

export default function StatusBadge({ status }: StatusBadgeProps) {
  return (
    <span
      className={`status-badge ${status ? 'status-badge--success' : 'status-badge--error'}`}
      title={status ? 'Transaction succeeded' : 'Transaction failed'}
    >
      {status ? 'Success' : 'Failed'}
    </span>
  );
}
