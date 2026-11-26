import type { ReactNode } from 'react';

export interface EmptyStateProps {
  title: ReactNode;
  description?: ReactNode;
  icon?: ReactNode;
  action?: ReactNode;
}

export function EmptyState({ title, description, icon, action }: EmptyStateProps) {
  return (
    <div className="ui-empty" role="status">
      {icon && (
        <div className="ui-empty__icon" aria-hidden="true">
          {icon}
        </div>
      )}
      <p className="ui-empty__title">{title}</p>
      {description && <div className="ui-empty__desc">{description}</div>}
      {action && <div className="ui-empty__action">{action}</div>}
    </div>
  );
}
