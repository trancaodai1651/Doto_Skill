import { useMemo, useState } from 'react';
import { Button, EmptyState, TextInput } from '@doto/ui';
import { api } from '../api/client';
import { AsyncView, PageHeader } from '../components/common';
import { useI18n } from '../i18n';
import { formatDate } from '../lib/format';
import { useApi } from '../lib/useApi';

export function AuditPage() {
  const { t, lang } = useI18n();
  const audit = useApi('audit', () => api.system.audit(300));
  const [q, setQ] = useState('');
  const filtered = useMemo(() => {
    const needle = q.trim().toLowerCase();
    const l = audit.data ?? [];
    if (!needle) return l;
    return l.filter((e) => [e.actor, e.action, e.target ?? '', e.ip ?? ''].some((x) => x.toLowerCase().includes(needle)));
  }, [audit.data, q]);
  return (
    <>
      <PageHeader
        title={t('nav.audit')}
        description={t('audit.desc')}
        actions={
          <Button size="sm" onClick={() => void audit.reload()} loading={audit.loading}>
            {t('common.reload')}
          </Button>
        }
      />
      <div className="mb-4 max-w-md">
        <TextInput label={t('common.search')} type="search" value={q} onChange={(e) => setQ(e.target.value)} />
      </div>
      <AsyncView state={audit} isEmpty={() => filtered.length === 0} empty={<EmptyState title={t('audit.empty')} />}>
        {() => (
          <div className="overflow-x-auto">
            <table className="data">
              <thead>
                <tr>
                  <th>{t('audit.time')}</th>
                  <th>{t('audit.actor')}</th>
                  <th>{t('audit.action')}</th>
                  <th>{t('audit.target')}</th>
                  <th className="hidden md:table-cell">IP</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((e) => (
                  <tr key={e.id}>
                    <td className="whitespace-nowrap">{formatDate(e.ts, lang)}</td>
                    <td>{e.actor}</td>
                    <td className="font-mono text-xs">{e.action}</td>
                    <td className="break-all">
                      {e.target ?? '—'}
                      {e.detail && (
                        <details>
                          <summary className="cursor-pointer text-xs text-muted">{t('audit.detail')}</summary>
                          <pre className="text-xs whitespace-pre-wrap">{JSON.stringify(e.detail, null, 2)}</pre>
                        </details>
                      )}
                    </td>
                    <td className="hidden md:table-cell">{e.ip ?? '—'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </AsyncView>
    </>
  );
}
