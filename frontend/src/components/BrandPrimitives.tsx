import type { ReactNode } from 'react';

type EntityKind =
  | 'search'
  | 'block'
  | 'blocks'
  | 'transaction'
  | 'transactions'
  | 'address'
  | 'addresses'
  | 'token'
  | 'tokens'
  | 'nft'
  | 'nfts'
  | 'status'
  | 'faucet'
  | 'notfound';

interface PageHeroProps {
  eyebrow?: string;
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  meta?: ReactNode;
  visual?: ReactNode;
  compact?: boolean;
  className?: string;
}

export function PageHero({
  eyebrow,
  title,
  description,
  actions,
  meta,
  visual,
  compact = false,
  className = '',
}: PageHeroProps) {
  return (
    <section className={`page-hero ${compact ? 'page-hero--compact' : ''} ${className}`.trim()}>
      <div className="page-hero__copy">
        {eyebrow && <p className="kicker">{eyebrow}</p>}
        <h1 className="page-title">{title}</h1>
        {description && <p className="page-description">{description}</p>}
        {actions && <div className="page-hero__actions">{actions}</div>}
        {meta && <div className="page-hero__meta">{meta}</div>}
      </div>
      <div className="page-hero__visual">
        {visual ?? <BrandOrnament />}
      </div>
    </section>
  );
}

interface StatCardProps {
  label: ReactNode;
  value: ReactNode;
  hint?: ReactNode;
  className?: string;
}

export function StatCard({ label, value, hint, className = '' }: StatCardProps) {
  return (
    <div className={`metric-card ${className}`.trim()}>
      <p className="metric-card__label">{label}</p>
      <div className="metric-card__value">{value}</div>
      {hint && <p className="metric-card__hint">{hint}</p>}
    </div>
  );
}

interface SectionPanelProps {
  eyebrow?: string;
  title?: ReactNode;
  actions?: ReactNode;
  className?: string;
  children: ReactNode;
}

export function SectionPanel({
  eyebrow,
  title,
  actions,
  className = '',
  children,
}: SectionPanelProps) {
  return (
    <section className={`card section-panel ${className}`.trim()}>
      {(eyebrow || title || actions) && (
        <div className="section-panel__header">
          <div>
            {eyebrow && <p className="kicker">{eyebrow}</p>}
            {title && <h2 className="section-title">{title}</h2>}
          </div>
          {actions && <div>{actions}</div>}
        </div>
      )}
      {children}
    </section>
  );
}

interface EmptyStateProps {
  title: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  className?: string;
}

export function EmptyState({
  title,
  description,
  action,
  className = '',
}: EmptyStateProps) {
  return (
    <div className={`empty-state ${className}`.trim()}>
      <div className="empty-state__mark" />
      <div>
        <h3 className="empty-state__title">{title}</h3>
        {description && <p className="empty-state__description">{description}</p>}
      </div>
      {action && <div className="empty-state__action">{action}</div>}
    </div>
  );
}

export function BrandOrnament({ compact = false }: { compact?: boolean }) {
  return (
    <div className={`brand-ornament ${compact ? 'brand-ornament--compact' : ''}`}>
      <div className="brand-ornament__field" />
      <div className="brand-ornament__arc brand-ornament__arc--outer" />
      <div className="brand-ornament__arc brand-ornament__arc--middle" />
      <div className="brand-ornament__arc brand-ornament__arc--inner" />
      <div className="brand-ornament__label">
        <span>Atlas</span>
        <span>Explorer</span>
      </div>
    </div>
  );
}

export function EntityHeroVisual({ kind }: { kind: EntityKind }) {
  return (
    <div className="entity-hero-visual">
      <div className="entity-hero-visual__field" />
      <div className="entity-hero-visual__arc entity-hero-visual__arc--outer" />
      <div className="entity-hero-visual__arc entity-hero-visual__arc--middle" />
      <div className="entity-hero-visual__arc entity-hero-visual__arc--inner" />
      <div className="entity-hero-visual__icon">
        <EntityGlyph kind={kind} />
      </div>
    </div>
  );
}

function EntityGlyph({ kind }: { kind: EntityKind }) {
  const cls = 'h-16 w-16 md:h-20 md:w-20 text-fg';

  if (kind === 'blocks' || kind === 'block') {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
        <path strokeLinecap="round" strokeLinejoin="round" d="M12 3 4 7.2l8 4.2 8-4.2L12 3Z" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M4 11.5 12 15.7l8-4.2" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M4 15.8 12 20l8-4.2" />
      </svg>
    );
  }

  if (kind === 'transactions' || kind === 'transaction') {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
        <path strokeLinecap="round" strokeLinejoin="round" d="M4 8h11" />
        <path strokeLinecap="round" strokeLinejoin="round" d="m11 4 4 4-4 4" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M20 16H9" />
        <path strokeLinecap="round" strokeLinejoin="round" d="m13 12-4 4 4 4" />
      </svg>
    );
  }

  if (kind === 'addresses' || kind === 'address') {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
        <circle cx="12" cy="8" r="3.5" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M5 20c1.7-3.2 4-4.8 7-4.8S17.3 16.8 19 20" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M3.5 6.5h2M18.5 6.5h2" />
      </svg>
    );
  }

  if (kind === 'tokens' || kind === 'token') {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
        <circle cx="12" cy="12" r="6.5" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M12 5.5v13M8.5 8.5h7M8.5 15.5h7" />
      </svg>
    );
  }

  if (kind === 'nfts' || kind === 'nft') {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
        <rect x="5" y="5" width="14" height="14" rx="2.5" />
        <path strokeLinecap="round" strokeLinejoin="round" d="m8 15 2.8-3.2 2.6 2.2 2.6-3 2 4" />
        <circle cx="9" cy="9" r="1.2" fill="currentColor" stroke="none" />
      </svg>
    );
  }

  if (kind === 'status') {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
        <path strokeLinecap="round" strokeLinejoin="round" d="M5 16.5h3l2-4 3.2 6 2.2-4H19" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M5 5.5v13h14" />
      </svg>
    );
  }

  if (kind === 'faucet') {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
        <path strokeLinecap="round" strokeLinejoin="round" d="M7 6h8.5a2.5 2.5 0 1 1 0 5H13" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M7 6v6.5a4.5 4.5 0 0 0 9 0V11" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M12 18.5c0 1.1-.9 2-2 2" />
      </svg>
    );
  }

  if (kind === 'notfound') {
    return (
      <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
        <circle cx="12" cy="12" r="7" />
        <path strokeLinecap="round" strokeLinejoin="round" d="M9.5 9.5h.01M14.5 9.5h.01M9.5 15.5c.8-.8 1.63-1.2 2.5-1.2.87 0 1.7.4 2.5 1.2" />
      </svg>
    );
  }

  return (
    <svg className={cls} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.35">
      <circle cx="11" cy="11" r="5.5" />
      <path strokeLinecap="round" strokeLinejoin="round" d="m15 15 4 4" />
    </svg>
  );
}
