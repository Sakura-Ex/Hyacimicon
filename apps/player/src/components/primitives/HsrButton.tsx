import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { COIN_ICON_URL } from '../../assets/paths';

export type HsrButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'circle-close';
export type HsrButtonSize = 'sm' | 'md' | 'lg';

export interface HsrButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: HsrButtonVariant;
  size?: HsrButtonSize;
  icon?: ReactNode;
  cost?: number;
  children?: ReactNode;
}

export function HsrButton({
  variant = 'secondary',
  size = 'md',
  icon,
  cost,
  children,
  className,
  type = 'button',
  ...rest
}: HsrButtonProps) {
  const classes = [
    'hsr-btn',
    `hsr-btn--${variant}`,
    variant !== 'circle-close' ? `hsr-btn--${size}` : '',
    className ?? '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <button type={type} className={classes} {...rest}>
      {icon}
      {children}
      {cost !== undefined && (
        <span className="hsr-btn-cost">
          <img src={COIN_ICON_URL} alt="" aria-hidden="true" />
          <span>{cost}</span>
        </span>
      )}
    </button>
  );
}
