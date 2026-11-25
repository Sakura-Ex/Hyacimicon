import { Fragment } from 'react';

const TOKEN_REGEX = /(\d+(?:\.\d+)?%?|【[^】]+】|\[[^\]]+\])/g;

export interface HsrRichTextProps {
  text: string;
  className?: string;
}

export function HsrRichText({ text, className }: HsrRichTextProps) {
  const parts = text.split(TOKEN_REGEX);

  return (
    <span className={className}>
      {parts.map((part, index) => {
        if (part === '') {
          return null;
        }
        if (/^\d+(?:\.\d+)?%?$/.test(part)) {
          return (
            <span key={index} className="hsr-rich-num">
              {part}
            </span>
          );
        }
        if (/^(?:【[^】]+】|\[[^\]]+\])$/.test(part)) {
          return (
            <span key={index} className="hsr-rich-term">
              {part}
            </span>
          );
        }
        return <Fragment key={index}>{part}</Fragment>;
      })}
    </span>
  );
}
