import { useState } from 'react';
import type { FormEvent } from 'react';
import { Button, Tabs, TextInput } from '@doto/ui';
import { api, errorMessage, http } from '../api/client';
import { ServerConnectionBox } from '../components/common';
import { Toolbar } from '../components/Layout';
import { useT } from '../i18n';
import { guessDeviceName } from '../lib/format';
import { useSession } from '../state/session';

function readPairParam(): string {
  try {
    return new URLSearchParams(window.location.search).get('pair')?.trim().toUpperCase() ?? '';
  } catch {
    return '';
  }
}

function clearPairParam() {
  const url = new URL(window.location.href);
  if (url.searchParams.has('pair')) {
    url.searchParams.delete('pair');
    window.history.replaceState(window.history.state, '', url.pathname + url.search + url.hash);
  }
}

export function LoginPage() {
  const t = useT();
  const { setMe, refresh, desktopError } = useSession();
  const initialPair = readPairParam();
  const [tab, setTab] = useState(initialPair ? 'pair' : 'login');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [code, setCode] = useState(initialPair);
  const [deviceName, setDeviceName] = useState(guessDeviceName());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fieldErr, setFieldErr] = useState<Record<string, string>>({});

  async function onLogin(e: FormEvent) {
    e.preventDefault();
    const errs: Record<string, string> = {};
    if (!username.trim()) errs.username = t('login.usernameRequired');
    if (!password) errs.password = t('login.passwordRequired');
    setFieldErr(errs);
    if (Object.keys(errs).length) return;
    setBusy(true);
    setError(null);
    try {
      const authMode = http.isCrossOrigin() ? 'bearer' : undefined;
      const me = await api.auth.login(username.trim(), password, deviceName.trim() || undefined, authMode);
      if (me && typeof me === 'object' && 'user' in me) setMe(me);
      await refresh();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function onPair(e: FormEvent) {
    e.preventDefault();
    const c = code.trim().toUpperCase();
    const errs: Record<string, string> = {};
    if (!c) errs.code = t('pair.codeRequired');
    if (!deviceName.trim()) errs.deviceName = t('pair.deviceRequired');
    setFieldErr(errs);
    if (Object.keys(errs).length) return;
    setBusy(true);
    setError(null);
    try {
      const authMode = http.isCrossOrigin() ? 'bearer' : undefined;
      const me = await api.auth.pair(c, deviceName.trim(), authMode);
      if (me && typeof me === 'object' && 'user' in me) setMe(me);
      clearPairParam();
      await refresh();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="rounded-xl border border-line bg-surface p-5 shadow-sm sm:p-6">
      <div className="mb-4 flex items-center justify-between gap-2">
        <h1 className="flex items-center gap-2 text-lg font-bold">
          <img src="/icons/icon.svg" alt="" width={28} height={28} /> Doto Skill
        </h1>
        <Toolbar />
      </div>
      {desktopError && (
        <p role="alert" className="mb-3 rounded-md bg-danger-bg p-3 text-sm text-danger">
          {t('login.desktopFailed')}: {desktopError}
        </p>
      )}
      <Tabs
        label={t('login.tabs')}
        value={tab}
        onChange={(v) => {
          setTab(v);
          setError(null);
          setFieldErr({});
        }}
        tabs={[
          { id: 'login', label: t('login.tab') },
          { id: 'pair', label: t('pair.tab') },
        ]}
      >
        {tab === 'login' ? (
          <form onSubmit={onLogin} className="flex flex-col gap-3" noValidate>
            <TextInput
              label={t('login.username')}
              autoComplete="username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              error={fieldErr.username}
              autoCapitalize="none"
              autoCorrect="off"
            />
            <TextInput
              label={t('login.password')}
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              error={fieldErr.password}
            />
            {error && (
              <p role="alert" className="text-sm font-semibold text-danger">
                {error}
              </p>
            )}
            <Button type="submit" variant="primary" loading={busy}>
              {t('login.submit')}
            </Button>
            <p className="text-xs text-muted">{t('login.help')}</p>
          </form>
        ) : (
          <form onSubmit={onPair} className="flex flex-col gap-3" noValidate>
            <p className="text-sm text-muted">{t('pair.intro')}</p>
            <TextInput
              label={t('pair.code')}
              value={code}
              onChange={(e) => setCode(e.target.value.toUpperCase())}
              error={fieldErr.code}
              autoCapitalize="characters"
              autoCorrect="off"
              autoComplete="one-time-code"
              maxLength={16}
              className="font-mono tracking-widest"
            />
            <TextInput
              label={t('pair.deviceName')}
              value={deviceName}
              onChange={(e) => setDeviceName(e.target.value)}
              error={fieldErr.deviceName}
              hint={t('pair.deviceHint')}
            />
            {error && (
              <p role="alert" className="text-sm font-semibold text-danger">
                {error}
              </p>
            )}
            <Button type="submit" variant="primary" loading={busy}>
              {t('pair.submit')}
            </Button>
          </form>
        )}
      </Tabs>
      <div className="mt-4">
        <ServerConnectionBox />
      </div>
    </div>
  );
}
