import { useState } from 'react';
import { toHsrAssetUrl } from '../../assets/paths';

export interface HsrAssetImageProps {
  src: string | null | undefined;
  alt?: string;
  className?: string;
  fallbackLabel?: string;
  loading?: 'eager' | 'lazy';
}

export function HsrAssetImage({
  src,
  alt = '',
  className,
  fallbackLabel,
  loading = 'lazy',
}: HsrAssetImageProps) {
  const resolved = toHsrAssetUrl(src);
  const [failed, setFailed] = useState(false);

  if (!resolved || failed) {
    return (
      <span className={`hsr-asset-fallback ${className ?? ''}`} aria-hidden={alt === ''}>
        <span>{fallbackLabel ? fallbackLabel.slice(0, 1) : '✦'}</span>
      </span>
    );
  }

  return (
    <img
      src={resolved}
      alt={alt}
      className={className}
      loading={loading}
      onError={() => setFailed(true)}
    />
  );
}
