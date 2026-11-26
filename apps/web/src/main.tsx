import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router';
import '@doto/ui/styles.css';
import './styles.css';
import { App } from './App';
import { I18nProvider } from './i18n';
import { ThemeProvider } from './state/theme';
import { SessionProvider } from './state/session';
import { ToastProvider } from '@doto/ui';

const root = document.getElementById('root');
if (!root) throw new Error('#root missing');

createRoot(root).render(
  <StrictMode>
    <ThemeProvider>
      <I18nProvider>
        <ToastProvider>
          <SessionProvider>
            <BrowserRouter>
              <App />
            </BrowserRouter>
          </SessionProvider>
        </ToastProvider>
      </I18nProvider>
    </ThemeProvider>
  </StrictMode>,
);
