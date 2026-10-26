/** The wordmark: a retained tree's three nodes, then the name. */
export function Logo() {
  return (
    <span className="inline-flex items-center gap-2 font-semibold tracking-tight">
      <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden="true" className="text-fd-primary">
        <rect x="2" y="2" width="20" height="20" rx="6" fill="currentColor" />
        <path d="M7.5 8h9l-9 8h9" fill="none" stroke="var(--color-fd-background)" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      zgx
    </span>
  );
}
