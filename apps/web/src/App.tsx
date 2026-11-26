import type { ReactNode } from 'react';
import { Navigate, Route, Routes, useLocation } from 'react-router';
import { Button } from '@doto/ui';
import { EventsProvider } from './api/events';
import { ErrorBlock, LoadingBlock } from './components/common';
import { AdminLayout, ViewerLayout } from './components/Layout';
import { useT } from './i18n';
import { useCaps, useSession } from './state/session';
import { LoginPage } from './pages/Login';
import { SetupPage } from './pages/Setup';
import { OnboardingPage } from './pages/Onboarding';
import { DashboardPage } from './pages/Dashboard';
import { SkillsPage } from './pages/Skills';
import { ToolsPage } from './pages/Tools';
import { VideosPage } from './pages/Videos';
import { JobsPage } from './pages/Jobs';
import { ServerPage } from './pages/Server';
import { DevicesPage } from './pages/Devices';
import { PerformancePage } from './pages/Performance';
import { SettingsPage } from './pages/Settings';
import { AuditPage } from './pages/Audit';
import { HelpPage } from './pages/Help';
import { GalleryPage, PlayerPage } from './pages/Gallery';

function CenterShell({ children }: { children: ReactNode }) {
  return <main className="mx-auto flex min-h-dvh w-full max-w-md flex-col justify-center px-4 py-8">{children}</main>;
}

export function App() {
  const { me, loading, error, refresh } = useSession();
  const { isOwner, hostControl } = useCaps();
  const location = useLocation();
  const t = useT();

  if (loading) {
    return (
      <CenterShell>
        <LoadingBlock label={t('app.starting')} />
      </CenterShell>
    );
  }
  if (!me) {
    return (
      <CenterShell>
        <ErrorBlock message={error ?? t('common.error')} onRetry={() => void refresh()} />
      </CenterShell>
    );
  }

  // Lần chạy đầu: owner cục bộ chưa có mật khẩu → cửa sổ desktop bắt buộc tạo tài khoản Owner
  // (mật khẩu này dùng khi đăng nhập từ thiết bị khác qua LAN/Tunnel).
  if (me.setupRequired && (!me.user || me.hostControl)) {
    return (
      <CenterShell>
        {me.hostControl ? (
          <SetupPage />
        ) : (
          <div className="rounded-xl border border-line bg-surface p-6 text-center">
            <h1 className="text-lg font-bold">{t('setup.title')}</h1>
            <p className="mt-2 text-sm text-muted">{t('setup.desktopOnly')}</p>
            <Button className="mt-4" onClick={() => void refresh()}>
              {t('common.retry')}
            </Button>
          </div>
        )}
      </CenterShell>
    );
  }

  if (!me.user) {
    return (
      <CenterShell>
        <LoginPage />
      </CenterShell>
    );
  }

  const role = me.user.role;

  if (isOwner && hostControl && !me.onboardingCompleted) {
    return (
      <EventsProvider role={role}>
        {location.pathname !== '/onboarding' ? <Navigate to="/onboarding" replace /> : <OnboardingPage />}
      </EventsProvider>
    );
  }

  if (!isOwner) {
    return (
      <EventsProvider role={role}>
        <Routes>
          <Route element={<ViewerLayout />}>
            <Route path="/gallery" element={<GalleryPage />} />
            <Route path="/gallery/:id" element={<PlayerPage />} />
            <Route path="*" element={<Navigate to="/gallery" replace />} />
          </Route>
        </Routes>
      </EventsProvider>
    );
  }

  return (
    <EventsProvider role={role}>
      <Routes>
        <Route path="/onboarding" element={hostControl ? <OnboardingPage /> : <Navigate to="/" replace />} />
        <Route element={<AdminLayout />}>
          <Route index element={<DashboardPage />} />
          <Route path="/skills" element={<SkillsPage />} />
          <Route path="/tools" element={<ToolsPage />} />
          <Route path="/videos" element={<VideosPage />} />
          <Route path="/jobs" element={<JobsPage />} />
          <Route path="/server" element={<ServerPage />} />
          <Route path="/devices" element={<DevicesPage />} />
          <Route path="/performance" element={<PerformancePage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/audit" element={<AuditPage />} />
          <Route path="/help" element={<HelpPage />} />
          <Route path="/gallery" element={<GalleryPage />} />
          <Route path="/gallery/:id" element={<PlayerPage />} />
          <Route path="/login" element={<Navigate to="/" replace />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </EventsProvider>
  );
}
