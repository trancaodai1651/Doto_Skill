import { useState } from 'react';
import { Button, Card, Checkbox, ConfirmDialog, Select, TextInput } from '@doto/ui';
import { api } from '../api/client';
import type { AppSettings, BackupEntry } from '../api/types';
import { AsyncView, Code, HostOnly, PageHeader, ServerConnectionBox, useNotify } from '../components/common';
import { useI18n } from '../i18n';
import type { Lang } from '../i18n';
import { formatBytes, formatDate } from '../lib/format';
import { useApi } from '../lib/useApi';
import { useCaps, useSession } from '../state/session';
import { useTheme } from '../state/theme';
import type { ThemePref } from '../state/theme';
import { FoldersForm } from './Onboarding';

export function SettingsPage() {
  const { t } = useI18n();
  const { isDesktopWindow } = useCaps();
  const settings = useApi('settings', () => api.settings.get());
  return (
    <>
      <PageHeader title={t('nav.settings')} description={t('settings.desc')} />
      <div className="flex flex-col gap-4">
        {!isDesktopWindow && (
          <Card title={t('conn.title')} description={t('conn.desc')}>
            <ServerConnectionBox />
          </Card>
        )}
        <AppearanceCard settings={settings.data} />
        <AsyncView state={settings}>
          {(s) => (
            <>
              <FoldersForm key={JSON.stringify([s.videoSourceDirs, s.outputDir])} settings={s} onSaved={(n) => settings.setData(n)} />
              <SystemCard key={JSON.stringify(s)} settings={s} onSaved={(n) => settings.setData(n)} />
            </>
          )}
        </AsyncView>
        <BackupsCard onRestored={() => void settings.reload()} />
        <AccountCard />
      </div>
    </>
  );
}

function AppearanceCard({ settings }: { settings: AppSettings | undefined }) {
  const { t, lang, setLang } = useI18n();
  const { theme, setTheme } = useTheme();
  const { hostControl } = useCaps();
  const sync = (patch: Partial<AppSettings>) => {
    if (hostControl && settings) void api.settings.put({ ...settings, ...patch }).catch(() => undefined);
  };
  return (
    <Card title={t('settings.appearance')} description={hostControl ? undefined : t('settings.localOnly')}>
      <div className="grid gap-3 sm:grid-cols-2">
        <Select
          label={t('toolbar.language')}
          value={lang}
          onChange={(e) => {
            const l = e.target.value as Lang;
            setLang(l);
            sync({ language: l });
          }}
          options={[
            { value: 'vi', label: 'Tiếng Việt' },
            { value: 'en', label: 'English' },
          ]}
        />
        <Select
          label={t('toolbar.theme')}
          value={theme}
          onChange={(e) => {
            const th = e.target.value as ThemePref;
            setTheme(th);
            sync({ theme: th });
          }}
          options={[
            { value: 'system', label: t('theme.system') },
            { value: 'light', label: t('theme.light') },
            { value: 'dark', label: t('theme.dark') },
          ]}
        />
      </div>
    </Card>
  );
}

function SystemCard({ settings, onSaved }: { settings: AppSettings; onSaved: (s: AppSettings) => void }) {
  const { t } = useI18n();
  const notify = useNotify();
  const { hostControl } = useCaps();
  const [ffmpegPath, setFfmpeg] = useState(settings.ffmpegPath ?? '');
  const [ffprobePath, setFfprobe] = useState(settings.ffprobePath ?? '');
  const [launchAtLogin, setLaunch] = useState(settings.launchAtLogin);
  const [runInBackground, setBg] = useState(settings.runInBackground);
  const [busy, setBusy] = useState(false);
  const [backupBusy, setBackupBusy] = useState(false);
  const [confirmBackup, setConfirmBackup] = useState(false);

  return (
    <Card title={t('settings.system')}>
      <form
        className="flex flex-col gap-3"
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          try {
            const next: AppSettings = {
              ...settings,
              ffmpegPath: ffmpegPath.trim() || null,
              ffprobePath: ffprobePath.trim() || null,
              launchAtLogin,
              runInBackground,
            };
            const r = await api.settings.put(next);
            onSaved(r && typeof r === 'object' && 'dataDir' in r ? r : next);
            notify.ok(t('common.saved'));
          } catch (e2) {
            notify.fail(e2);
          } finally {
            setBusy(false);
          }
        }}
      >
        <fieldset disabled={!hostControl} className="grid gap-3 sm:grid-cols-2">
          <TextInput label={t('settings.ffmpegPath')} value={ffmpegPath} onChange={(e) => setFfmpeg(e.target.value)} placeholder={t('settings.autoDetect')} spellCheck={false} />
          <TextInput label={t('settings.ffprobePath')} value={ffprobePath} onChange={(e) => setFfprobe(e.target.value)} placeholder={t('settings.autoDetect')} spellCheck={false} />
          <Checkbox label={t('settings.launchAtLogin')} checked={launchAtLogin} onChange={(e) => setLaunch(e.target.checked)} />
          <Checkbox label={t('settings.runInBackground')} checked={runInBackground} onChange={(e) => setBg(e.target.checked)} />
        </fieldset>
        <p className="text-sm">
          {t('onb.terms.dataDir')}: <Code>{settings.dataDir}</Code>
        </p>
        <div className="flex flex-wrap gap-2">
          <HostOnly>
            <Button type="submit" variant="primary" loading={busy}>
              {t('common.save')}
            </Button>
            <Button loading={backupBusy} onClick={() => setConfirmBackup(true)}>
              {t('settings.backup')}
            </Button>
          </HostOnly>
        </div>
      </form>
      <ConfirmDialog
        open={confirmBackup}
        title={t('settings.backup')}
        confirmLabel={t('settings.backup')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirmBackup(false)}
        onConfirm={async () => {
          setBackupBusy(true);
          try {
            const r = await api.system.backup();
            setConfirmBackup(false);
            const fileLabel = r && typeof r === 'object' ? (r.file ?? r.path) : undefined;
            notify.ok(t('settings.backupDone'), fileLabel);
          } finally {
            setBackupBusy(false);
          }
        }}
      >
        <p>{t('settings.backupBody')}</p>
      </ConfirmDialog>
    </Card>
  );
}

function BackupsCard({ onRestored }: { onRestored: () => void }) {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const backups = useApi('backups', () => api.system.backups());
  const [creating, setCreating] = useState(false);
  const [restoreTarget, setRestoreTarget] = useState<BackupEntry | null>(null);

  return (
    <Card
      title={t('settings.backupsTitle')}
      description={t('settings.backupsDesc')}
      actions={
        <div className="flex gap-2">
          <Button size="sm" onClick={() => void backups.reload()} loading={backups.loading}>
            {t('common.reload')}
          </Button>
          <Button
            size="sm"
            variant="primary"
            loading={creating}
            onClick={async () => {
              setCreating(true);
              try {
                const r = await api.system.backup();
                notify.ok(t('settings.backupDone'), r?.file);
                await backups.reload();
              } catch (e) {
                notify.fail(e);
              } finally {
                setCreating(false);
              }
            }}
          >
            {t('settings.backup')}
          </Button>
        </div>
      }
    >
      <AsyncView state={backups} isEmpty={(l) => l.length === 0} empty={<p className="text-sm text-muted">{t('settings.backupsEmpty')}</p>}>
        {(list) => (
          <ul className="divide-y divide-line text-sm">
            {list.map((b) => (
              <li key={b.file} className="flex flex-wrap items-center justify-between gap-2 py-2">
                <div>
                  <Code>{b.file}</Code>
                  <span className="ml-2 text-xs text-muted">
                    {formatBytes(b.sizeBytes)} · {formatDate(b.createdAt, lang)}
                  </span>
                </div>
                <Button size="sm" variant="secondary" onClick={() => setRestoreTarget(b)}>
                  {t('settings.restore')}
                </Button>
              </li>
            ))}
          </ul>
        )}
      </AsyncView>
      <ConfirmDialog
        open={restoreTarget !== null}
        tone="danger"
        title={t('settings.restoreTitle', { file: restoreTarget?.file ?? '' })}
        confirmLabel={t('settings.restore')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setRestoreTarget(null)}
        onConfirm={async () => {
          if (!restoreTarget) return;
          await api.system.restore(restoreTarget.file);
          setRestoreTarget(null);
          notify.ok(t('settings.restored'));
          onRestored();
          await backups.reload();
        }}
      >
        <p>{t('settings.restoreBody')}</p>
      </ConfirmDialog>
    </Card>
  );
}

function AccountCard() {
  const { t } = useI18n();
  const { me } = useSession();
  return (
    <Card title={t('settings.account')}>
      <p className="text-sm">
        {t('settings.signedInAs', { name: me?.user?.username ?? '' })} · {me?.hostControl ? t('settings.desktopSession') : t('settings.remoteSession')} · v
        {me?.version}
      </p>
      <p className="mt-1 text-xs text-muted">{t('settings.logoutHint')}</p>
    </Card>
  );
}
