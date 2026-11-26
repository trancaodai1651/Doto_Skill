import type { HTMLAttributes, ReactNode } from 'react';
import { useId } from 'react';
import { cx } from './cx';

export interface CardProps extends Omit<HTMLAttributes<HTMLElement>, 'title'> {
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}

export function Card({ title, description, actions, className, children, ...rest }: CardProps) {
  const id = useId();
  return (
    <section className={cx('ui-card', className)} aria-labelledby={title ? id : undefined} {...rest}>
      {(title || actions) && (
        <div className="ui-card__head">
          <div>
            {title && (
              <h2 className="ui-card__title" id={id}>
                {title}
              </h2>
            )}
            {description && <p className="ui-card__desc">{description}</p>}
          </div>
          {actions && <div className="ui-card__actions">{actions}</div>}
        </div>
      )}
      {children}
    </section>
  );
}
