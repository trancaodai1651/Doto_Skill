import { Card } from '@doto/ui';
import { Code, PageHeader } from '../components/common';
import { useI18n } from '../i18n';
import type { TKey } from '../i18n';

const SECTIONS: Array<{ title: TKey; items: TKey[] }> = [
  { title: 'help.start.title', items: ['help.start.1', 'help.start.2', 'help.start.3'] },
  { title: 'help.skills.title', items: ['help.skills.1', 'help.skills.2', 'help.skills.3', 'help.skills.4'] },
  { title: 'help.remote.title', items: ['help.remote.1', 'help.remote.2', 'help.remote.3', 'help.remote.4'] },
  { title: 'help.video.title', items: ['help.video.1', 'help.video.2', 'help.video.3'] },
  { title: 'help.security.title', items: ['help.security.1', 'help.security.2', 'help.security.3'] },
];

export function HelpPage() {
  const { t } = useI18n();
  return (
    <>
      <PageHeader title={t('nav.help')} description={t('help.desc')} />
      <div className="grid gap-4 md:grid-cols-2">
        {SECTIONS.map((s) => (
          <Card key={s.title} title={t(s.title)}>
            <ul className="list-disc space-y-1.5 pl-5 text-sm">
              {s.items.map((i) => (
                <li key={i}>{t(i)}</li>
              ))}
            </ul>
          </Card>
        ))}
        <Card title={t('help.locations.title')}>
          <p className="mb-2 text-sm">{t('help.locations.desc')}</p>
          <ul className="space-y-1.5 text-sm">
            <li>
              Claude Code: <Code>~/.claude/skills/&lt;name&gt;/SKILL.md</Code>
            </li>
            <li>
              Codex: <Code>~/.agents/skills</Code> — {t('help.locations.codex')}
            </li>
            <li>
              Antigravity: <Code>~/.gemini/config/skills/</Code> — {t('help.locations.antigravity')}
            </li>
          </ul>
          <p className="mt-2 text-xs text-muted">{t('help.locations.note')}</p>
        </Card>
      </div>
    </>
  );
}
