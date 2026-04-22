import { Link } from 'react-router-dom';
import { EmptyState, EntityHeroVisual, PageHero } from '../components';

export default function NotFoundPage() {
  return (
    <div className="space-y-6 fade-in-up">
      <PageHero
        compact
        title="404"
        visual={<EntityHeroVisual kind="notfound" />}
      />
      <EmptyState
        title="Page not found"
        description="Return to the homepage and continue from search, or navigate directly into the explorer sections."
        action={
          <Link to="/" className="btn btn-primary">
            Go home
          </Link>
        }
      />
    </div>
  );
}
