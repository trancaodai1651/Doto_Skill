import { useState } from 'react';
import type { FormEvent } from 'react';
import { Button, TextInput } from '@doto/ui';
import { api, errorMessage, http } from '../api/client';
import { Toolbar } from '../components/Layout';
import { useT } from '../i18n';
import { useSession } from '../state/session';

/** Same rule as the server: ≥ 10 chars, contains a letter and a digit. */
export function validateOwnerPassword(pw: string): 'short' | 'weak' | null {
  if (pw.length < 10) return 'short';
  if (!/[A-Za-zÀ-ỹ]/.test(pw) || !/\d/.test(pw)) return 'weak';
  return null;
}

export function SetupPage() {
  const t = useT();
  const { refresh, setMe } = useSession();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [errs, setErrs] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const next: Record<string, string> = {};
    if (!/^[A-Za-z0-9._-]{3,32}$/.test(username.trim())) next.username = t('setup.usernameRule');
    const pw = validateOwnerPassword(password);
    if (pw) next.password = t(pw === 'short' ? 'setup.pwShort' : 'setup.pwWeak');
    if (confirm !== password) next.confirm = t('setup.pwMismatch');
    setErrs(next);
    if (Object.keys(next).length) return;
    setBusy(true);
    setError(null);
    try {
      const authMode = http.isCrossOrigin() ? 'bearer' : undefined;
      const res = await api.auth.setup(username.trim(), password, authMode);
      if (res && typeof res === 'object' && 'user' in res) setMe(res);
      await refresh();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="rounded-xl border border-line bg-surface p-5 shadow-sm sm:p-6">
      <div className="mb-3 flex items-center justify-between gap-2">
        <h1 className="text-lg font-bold">{t('setup.title')}</h1>
        <Toolbar />
      </div>
      <p className="mb-4 text-sm text-muted">{t('setup.intro')}</p>
      <form onSubmit={onSubmit} className="flex flex-col gap-3" noValidate>
        <TextInput label={t('login.username')} value={username} onChange={(e) => setUsername(e.target.value)} error={errs.username} autoComplete="username" autoCapitalize="none" />
        <TextInput
          label={t('login.password')}
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          error={errs.password}
          hint={t('setup.pwHint')}
          autoComplete="new-password"
        />
        <TextInput label={t('setup.confirm')} type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} error={errs.confirm} autoComplete="new-password" />
        {error && (
          <p role="alert" className="text-sm font-semibold text-danger">
            {error}
          </p>
        )}
        <Button type="submit" variant="primary" loading={busy}>
          {t('setup.submit')}
        </Button>
      </form>
    </div>
  );
}
