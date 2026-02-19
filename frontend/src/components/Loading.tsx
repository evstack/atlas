interface LoadingProps {
  size?: 'sm' | 'md' | 'lg';
  text?: string;
}

export default function Loading({ size = 'md', text }: LoadingProps) {
  const sizeClasses = {
    sm: 'w-4 h-4 border-2',
    md: 'w-8 h-8 border-2',
    lg: 'w-12 h-12 border-3',
  };

  return (
    <div className="flex flex-col items-center justify-center py-12">
      <div
        className={`${sizeClasses[size]} rounded-full border-transparent border-t-accent-primary/90 border-l-dark-600/60 border-r-dark-600/60 animate-spin`}
      />
      {text && <p className="mt-4 text-gray-500 text-sm">{text}</p>}
    </div>
  );
}
