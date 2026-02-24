import { Link } from 'react-router-dom';

export default function NotFoundPage() {
  return (
    <div className="flex flex-col items-center justify-center py-20">
      <h1 className="text-6xl font-bold text-fg mb-4">404</h1>
      <p className="text-fg-subtle text-lg mb-8">Page not found</p>
      <Link to="/" className="btn btn-primary">
        Go Home
      </Link>
    </div>
  );
}
