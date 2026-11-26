import { useState } from 'react';
import { NavLink, Outlet, useLocation } from 'react-router';
import { Button, ConfirmDialog, Modal, cx } from '@doto/ui';
import { useEvents } from '../api/events';
import { useI18n } from '../i18n';
import type { TKey } from '../i18n';
import { useCaps, useSession } from '../state/session';
import { useTheme } from '../state/theme';
import type { ThemePref } from '../state/theme';

interface NavItem {
  to: string;
  key: TKey;
  icon: string;
}

export const ADMIN_NAV: NavItem[] = [
  { to: '/', key: 'nav.dashboard', icon: '◧' },
  { to: '/skills', key: 'nav.skills', icon: '✦' },
  { to: '/tools', key: 'nav.tools', icon: '⌘' },
  { to: '/videos', key: 'nav.videos', icon: '▶' },
  { to: '/jobs', key: 'nav.jobs', icon: '≡' },
  { to: '/server', key: 'nav.server', icon: '⇄' },
  { to: '/devices', key: 'nav.devices', icon: '▢' },
  { to: '/performance', key: 'nav.performance', icon: '▲' },
  { to: '/gallery', key: 'nav.gallery', icon: '▦' },
  { to: '/settings', key: 'nav.settings', icon: '⚙' },
  { to: '/audit', key: 'nav.audit', icon: '☰' },
  { to: '/help', key: 'nav.help', icon: '?' },
];

const MOBILE_PRIMARY = ['/', '/skills', '/videos', '/jobs'];

export function Toolbar() {
  const { lang, setLang, t } = useI18n();
  const { theme, setTheme } = useTheme();
  const { me, logout } = useSession();
  const [confirmLogout, setConfirmLogout] = useState(false);
  const nextTheme: Record<ThemePref, ThemePref> = { system: 'light', light: 'dark', dark: 'system' };
  const themeIcon: Record<ThemePref, string> = { system: '◐', light: '☀', dark: '☾' };
  return (
    <div className="flex items-center gap-1">
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setLang(lang === 'vi' ? 'en' : 'vi')}
        aria-label={t('toolbar.language')}
        title={t('toolbar.language')}
      >
        {lang === 'vi' ? 'VI' : 'EN'}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setTheme(nextTheme[theme])}
        aria-label={`${t('toolbar.theme')}: ${t(`theme.${theme}` as TKey)}`}
        title={`${t('toolbar.theme')}: ${t(`theme.${theme}` as TKey)}`}
      >
        <span aria-hidden="true">{themeIcon[theme]}</span>
      </Button>
      {me?.user && (
        <>
          <span className="hidden max-w-[10rem] truncate px-2 text-sm text-muted sm:inline" title={me.user.username}>
            {me.user.username}
          </span>
          <Button variant="ghost" size="sm" onClick={() => setConfirmLogout(true)}>
            {t('toolbar.logout')}
          </Button>
        </>
      )}
      <ConfirmDialog
        open={confirmLogout}
        title={t('logout.title')}
        confirmLabel={t('toolbar.logout')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirmLogout(false)}
        onConfirm={async () => {
          await logout();
          setConfirmLogout(false);
        }}
      >
        <p>{t('logout.body')}</p>
      </ConfirmDialog>
    </div>
  );
}

function RealtimeHint() {
  const { mode } = useEvents();
  const { t } = useI18n();
  if (mode !== 'polling') return null;
  return (
    <span
      className="rounded-full bg-warning-bg px-2 py-0.5 text-xs font-semibold whitespace-nowrap text-warning"
      role="status"
      aria-label={t('realtime.polling')}
      title={t('realtime.pollingHelp')}
    >
      <span aria-hidden="true">⟳</span>
      <span className="hidden sm:inline"> {t('realtime.polling')}</span>
    </span>
  );
}

function Brand() {
  return (
    <span className="flex items-center gap-2 font-bold whitespace-nowrap">
      <img src="/icons/icon.svg" alt="" width={24} height={24} />
      Doto Skill
    </span>
  );
}

export function AdminLayout() {
  const { t } = useI18n();
  const { remoteOwner } = useCaps();
  const [moreOpen, setMoreOpen] = useState(false);
  const location = useLocation();
  const linkClass = ({ isActive }: { isActive: boolean }) =>
    cx(
      'flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium',
      isActive ? 'bg-info-bg text-primary' : 'text-fg hover:bg-surface-2',
    );
  return (
    <div className="min-h-dvh md:flex">
      <a href="#main" className="skip-link ui-btn ui-btn--primary">
        {t('a11y.skip')}
      </a>
      <aside className="sticky top-0 hidden h-dvh w-60 shrink-0 flex-col border-r border-line bg-surface p-3 md:flex" aria-label={t('nav.label')}>
        <div className="mb-4 px-2 py-1">
          <Brand />
        </div>
        <nav className="flex flex-1 flex-col gap-0.5 overflow-y-auto" aria-label={t('nav.label')}>
          {ADMIN_NAV.map((n) => (
            <NavLink key={n.to} to={n.to} end={n.to === '/'} className={linkClass}>
              <span aria-hidden="true" className="w-4 text-center">
                {n.icon}
              </span>
              {t(n.key)}
            </NavLink>
          ))}
        </nav>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="sticky top-0 z-20 flex items-center justify-between gap-2 border-b border-line bg-surface/95 px-4 py-2 backdrop-blur">
          <div className="md:hidden">
            <Brand />
          </div>
          <div className="hidden md:block">
            <RealtimeHint />
          </div>
          <div className="flex items-center gap-2">
            <span className="md:hidden">
              <RealtimeHint />
            </span>
            <Toolbar />
          </div>
        </header>
        {remoteOwner && (
          <div className="border-b border-line bg-warning-bg px-4 py-2 text-sm text-warning" role="note" data-testid="remote-owner-banner">
            {t('host.remoteBanner')}
          </div>
        )}
        <main id="main" className="pb-safe mx-auto w-full max-w-6xl flex-1 px-4 pt-5 md:px-6" tabIndex={-1}>
          <Outlet />
        </main>
      </div>

      <nav
        className="bottom-nav fixed inset-x-0 bottom-0 z-30 grid grid-cols-5 border-t border-line bg-surface md:hidden"
        aria-label={t('nav.mobile')}
      >
        {ADMIN_NAV.filter((n) => MOBILE_PRIMARY.includes(n.to)).map((n) => (
          <NavLink
            key={n.to}
            to={n.to}
            end={n.to === '/'}
            className={({ isActive }) =>
              cx('flex flex-col items-center gap-0.5 py-2 text-[0.7rem] font-medium', isActive ? 'text-primary' : 'text-muted')
            }
          >
            <span aria-hidden="true" className="text-base">
              {n.icon}
            </span>
            {t(n.key)}
          </NavLink>
        ))}
        <button
          type="button"
          className={cx(
            'flex flex-col items-center gap-0.5 py-2 text-[0.7rem] font-medium',
            !MOBILE_PRIMARY.includes(location.pathname) ? 'text-primary' : 'text-muted',
          )}
          onClick={() => setMoreOpen(true)}
          aria-haspopup="dialog"
        >
          <span aria-hidden="true" className="text-base">
            ⋯
          </span>
          {t('nav.more')}
        </button>
      </nav>
      <Modal open={moreOpen} onClose={() => setMoreOpen(false)} title={t('nav.more')} closeLabel={t('common.close')}>
        <nav className="grid grid-cols-2 gap-2" aria-label={t('nav.more')}>
          {ADMIN_NAV.filter((n) => !MOBILE_PRIMARY.includes(n.to)).map((n) => (
            <NavLink key={n.to} to={n.to} className={linkClass} onClick={() => setMoreOpen(false)}>
              <span aria-hidden="true">{n.icon}</span>
              {t(n.key)}
            </NavLink>
          ))}
        </nav>
      </Modal>
    </div>
  );
}

export function ViewerLayout() {
  const { t } = useI18n();
  return (
    <div className="min-h-dvh">
      <a href="#main" className="skip-link ui-btn ui-btn--primary">
        {t('a11y.skip')}
      </a>
      <header className="sticky top-0 z-20 flex items-center justify-between gap-2 border-b border-line bg-surface/95 px-4 py-2 backdrop-blur">
        <NavLink to="/gallery" aria-label={t('nav.gallery')}>
          <Brand />
        </NavLink>
        <div className="flex items-center gap-2">
          <RealtimeHint />
          <Toolbar />
        </div>
      </header>
      <main id="main" className="mx-auto w-full max-w-6xl px-4 pt-5 pb-10 md:px-6" tabIndex={-1}>
        <Outlet />
      </main>
    </div>
  );
}
