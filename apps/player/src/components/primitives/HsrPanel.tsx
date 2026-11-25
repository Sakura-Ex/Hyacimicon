import type { ReactNode } from 'react';

export interface HsrPanelProps {
  title?: string;
  eyebrow?: string;
  rightSlot?: ReactNode;
  accent?: 'default' | 'gold' | 'violet' | 'prismatic';
  corners?: boolean;
  className?: string;
  bodyClassName?: string;
  children: ReactNode;
}

export function HsrPanel({
  title,
  eyebrow,
  rightSlot,
  accent = 'default',
  corners = true,
  className,
  bodyClassName,
  children,
}: HsrPanelProps) {
  const accentClass = accent === 'default' ? '' : `hsr-panel--${accent}`;
  const hasHeader = Boolean(title || eyebrow || rightSlot);

  return (
    <section className={`hsr-panel ${accentClass} ${className ?? ''}`.trim()}>
      {corners && (
        <>
          <span className="hsr-panel-corner hsr-panel-corner--tl" aria-hidden="true" />
          <span className="hsr-panel-corner hsr-panel-corner--tr" aria-hidden="true" />
          <span className="hsr-panel-corner hsr-panel-corner--bl" aria-hidden="true" />
          <span className="hsr-panel-corner hsr-panel-corner--br" aria-hidden="true" />
        </>
      )}
      {hasHeader && (
        <header className="hsr-panel-head">
          <div>
            {eyebrow && <div className="hsr-panel-eyebrow">{eyebrow}</div>}
            {title && <h3 className="hsr-panel-title">{title}</h3>}
          </div>
          {rightSlot}
        </header>
      )}
      <div className={`hsr-panel-body ${bodyClassName ?? ''}`.trim()}>{children}</div>
    </section>
  );
}
