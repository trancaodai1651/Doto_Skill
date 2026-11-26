import { useState } from 'react';
import { Badge, Button, Card, ConfirmDialog, EmptyState, Modal, Select, TextInput } from '@doto/ui';
import { api } from '../api/client';
import type { DeviceSession, PairingCode, Role, User } from '../api/types';
import { AsyncView, Code, ExternalLink, PageHeader, QrImage, useNotify } from '../components/common';
import { useI18n } from '../i18n';
import { formatDate } from '../lib/format';
import { useApi } from '../lib/useApi';
import { useSession } from '../state/session';
import { validateOwnerPassword } from './Setup';

export function DevicesPage() {
  const { t } = useI18n();
  return (
    <>
      <PageHeader title={t('nav.devices')} description={t('devices.desc')} />
      <div className="flex flex-col gap-4">
        <PairingCard />
        <SessionsCard />
        <UsersCard />
      </div>
    </>
  );
}

function SessionsCard() {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const { refresh } = useSession();
  const sessions = useApi('devices', () => api.devices.list());
  const [revoke, setRevoke] = useState<DeviceSession | null>(null);
  return (
    <Card
      title={t('devices.sessions')}
      actions={
        <Button size="sm" onClick={() => void sessions.reload()} loading={sessions.loading}>
          {t('common.reload')}
        </Button>
      }
    >
      <AsyncView state={sessions} isEmpty={(l) => l.length === 0} empty={<EmptyState title={t('devices.noSessions')} />}>
        {(list) => (
          <div className="overflow-x-auto">
            <table className="data">
              <thead>
                <tr>
                  <th>{t('devices.device')}</th>
                  <th>{t('devices.user')}</th>
                  <th className="hidden md:table-cell">IP</th>
                  <th className="hidden sm:table-cell">{t('devices.lastSeen')}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {list.map((d) => (
                  <tr key={d.id}>
                    <td>
                      <span className="font-medium">{d.deviceName}</span>
                      <span className="ml-1 inline-flex flex-wrap gap-1">
                        {d.current && <Badge tone="info">{t('devices.current')}</Badge>}
                        {d.hostControl && <Badge tone="success">{t('devices.desktop')}</Badge>}
                      </span>
                      {d.userAgent && <p className="max-w-xs truncate text-xs text-muted" title={d.userAgent}>{d.userAgent}</p>}
                    </td>
                    <td>
                      {d.username} <Badge>{t(`role.${d.role}`)}</Badge>
                    </td>
                    <td className="hidden md:table-cell">{d.ip ?? '—'}</td>
                    <td className="hidden sm:table-cell">{formatDate(d.lastSeenAt, lang)}</td>
                    <td>
                      <Button size="sm" variant="ghost" onClick={() => setRevoke(d)} aria-label={t('devices.revokeNamed', { name: d.deviceName })}>
                        {t('devices.revoke')}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </AsyncView>
      <ConfirmDialog
        open={revoke !== null}
        tone="danger"
        title={t('devices.revokeTitle', { name: revoke?.deviceName ?? '' })}
        confirmLabel={t('devices.revoke')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setRevoke(null)}
        onConfirm={async () => {
          if (!revoke) return;
          await api.devices.revoke(revoke.id);
          const wasCurrent = revoke.current;
          setRevoke(null);
          notify.ok(t('devices.revoked'));
          if (wasCurrent) await refresh();
          else await sessions.reload();
        }}
      >
        <p>{revoke?.current ? t('devices.revokeCurrent') : t('devices.revokeBody')}</p>
      </ConfirmDialog>
    </Card>
  );
}

function PairingCard() {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const [role, setRole] = useState<Role>('viewer');
  const [ttl, setTtl] = useState('15');
  const [code, setCode] = useState<PairingCode | null>(null);
  const [busy, setBusy] = useState(false);
  return (
    <Card title={t('pairing.title')} description={t('pairing.desc')}>
      <div className="grid gap-3 sm:grid-cols-3 sm:items-end">
        <Select
          label={t('pairing.role')}
          value={role}
          onChange={(e) => setRole(e.target.value as Role)}
          options={[
            { value: 'viewer', label: t('role.viewer') },
            { value: 'owner', label: t('role.owner') },
          ]}
        />
        <Select
          label={t('pairing.ttl')}
          value={ttl}
          onChange={(e) => setTtl(e.target.value)}
          options={['5', '15', '60', '1440'].map((m) => ({ value: m, label: t('pairing.minutes', { n: m }) }))}
        />
        <Button
          variant="primary"
          loading={busy}
          onClick={async () => {
            setBusy(true);
            try {
              setCode(await api.devices.createPairing(role, Number(ttl)));
            } catch (e) {
              notify.fail(e);
            } finally {
              setBusy(false);
            }
          }}
        >
          {t('pairing.create')}
        </Button>
      </div>
      {role === 'owner' && <p className="mt-2 text-sm text-warning">⚠ {t('pairing.ownerWarning')}</p>}
      <Modal open={code !== null} onClose={() => setCode(null)} title={t('pairing.codeTitle')} closeLabel={t('common.close')} footer={<Button onClick={() => setCode(null)}>{t('common.close')}</Button>}>
        {code && (
          <div className="flex flex-col items-center gap-3 text-center">
            <p className="font-mono text-3xl font-bold tracking-[0.3em]" data-testid="pairing-code">
              {code.code}
            </p>
            <p className="text-sm text-muted">
              {t('pairing.expires', { time: formatDate(code.expiresAt, lang) })} · {t(`role.${code.role}`)}
            </p>
            {(code.qrSvg || code.url) && <QrImage svg={code.qrSvg} src={code.url ? api.server.qrUrl(code.url) : undefined} alt={t('pairing.qrAlt')} />}
            {code.url ? <ExternalLink href={code.url}>{code.url}</ExternalLink> : <p className="text-sm text-muted">{t('pairing.noUrl')}</p>}
            <p className="text-xs text-muted">{t('pairing.once')}</p>
          </div>
        )}
      </Modal>
    </Card>
  );
}

function UsersCard() {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const { me } = useSession();
  const users = useApi('users', () => api.users.list());
  const [del, setDel] = useState<User | null>(null);
  const [pw, setPw] = useState<User | null>(null);
  const [newPw, setNewPw] = useState('');
  const [pwErr, setPwErr] = useState<string | undefined>();
  const [form, setForm] = useState({ username: '', password: '', role: 'viewer' as Role });
  const [errs, setErrs] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);

  return (
    <Card title={t('users.title')}>
      <AsyncView state={users} isEmpty={(l) => l.length === 0} empty={<EmptyState title={t('users.empty')} />}>
        {(list) => (
          <ul className="mb-4 flex flex-col divide-y divide-line">
            {list.map((u) => (
              <li key={u.id} className="flex flex-wrap items-center justify-between gap-2 py-2">
                <span>
                  <span className="font-medium">{u.username}</span> <Badge>{t(`role.${u.role}`)}</Badge>
                  <span className="ml-2 text-xs text-muted">{formatDate(u.createdAt, lang)}</span>
                </span>
                <span className="flex gap-2">
                  <Button size="sm" variant="ghost" onClick={() => setPw(u)}>
                    {t('users.changePassword')}
                  </Button>
                  {u.id !== me?.user?.id && (
                    <Button size="sm" variant="ghost" onClick={() => setDel(u)}>
                      {t('common.delete')}
                    </Button>
                  )}
                </span>
              </li>
            ))}
          </ul>
        )}
      </AsyncView>
      <form
        className="grid gap-3 sm:grid-cols-4 sm:items-start"
        onSubmit={async (e) => {
          e.preventDefault();
          const next: Record<string, string> = {};
          if (!/^[A-Za-z0-9._-]{3,32}$/.test(form.username.trim())) next.username = t('setup.usernameRule');
          const p = validateOwnerPassword(form.password);
          if (p) next.password = t(p === 'short' ? 'setup.pwShort' : 'setup.pwWeak');
          setErrs(next);
          if (Object.keys(next).length) return;
          setBusy(true);
          try {
            await api.users.create(form.username.trim(), form.password, form.role);
            setForm({ username: '', password: '', role: 'viewer' });
            notify.ok(t('users.created'));
            await users.reload();
          } catch (e2) {
            notify.fail(e2);
          } finally {
            setBusy(false);
          }
        }}
      >
        <TextInput label={t('login.username')} value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })} error={errs.username} autoComplete="off" />
        <TextInput label={t('login.password')} type="password" value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })} error={errs.password} autoComplete="new-password" />
        <Select
          label={t('pairing.role')}
          value={form.role}
          onChange={(e) => setForm({ ...form, role: e.target.value as Role })}
          options={[
            { value: 'viewer', label: t('role.viewer') },
            { value: 'owner', label: t('role.owner') },
          ]}
        />
        <Button type="submit" variant="primary" loading={busy} className="sm:mt-6">
          {t('users.create')}
        </Button>
      </form>
      <ConfirmDialog
        open={del !== null}
        tone="danger"
        title={t('users.deleteTitle', { name: del?.username ?? '' })}
        confirmLabel={t('common.delete')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setDel(null)}
        onConfirm={async () => {
          if (!del) return;
          await api.users.remove(del.id);
          setDel(null);
          await users.reload();
        }}
      >
        <p>{t('users.deleteBody')}</p>
      </ConfirmDialog>
      <ConfirmDialog
        open={pw !== null}
        title={t('users.changePasswordFor', { name: pw?.username ?? '' })}
        confirmLabel={t('common.save')}
        cancelLabel={t('common.cancel')}
        onCancel={() => {
          setPw(null);
          setNewPw('');
          setPwErr(undefined);
        }}
        onConfirm={async () => {
          if (!pw) return;
          const p = validateOwnerPassword(newPw);
          if (p) {
            setPwErr(t(p === 'short' ? 'setup.pwShort' : 'setup.pwWeak'));
            return;
          }
          await api.users.setPassword(pw.id, newPw);
          setPw(null);
          setNewPw('');
          setPwErr(undefined);
          notify.ok(t('common.saved'));
        }}
      >
        <TextInput label={t('users.newPassword')} type="password" value={newPw} onChange={(e) => setNewPw(e.target.value)} error={pwErr} autoComplete="new-password" />
        <p className="mt-2 text-xs text-muted">
          <Code>{t('setup.pwHint')}</Code>
        </p>
      </ConfirmDialog>
    </Card>
  );
}
