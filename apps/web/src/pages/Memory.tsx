import { useEffect, useState } from 'react';
import { Badge, Button, Card, Checkbox, ConfirmDialog, Select, TextArea, TextInput } from '@doto/ui';
import { api } from '../api/client';
import type { AiProviderId, MemoryDocStatus, MemoryOverview } from '../api/types';
import { AsyncView, Code, HostOnly, PageHeader, useNotify } from '../components/common';
import { useI18n } from '../i18n';
import { formatBytes, formatDate } from '../lib/format';
import { useApi } from '../lib/useApi';

export function MemoryPage() {
  const { t, lang } = useI18n();
  const notify = useNotify();
  const overview = useApi('memory-overview', () => api.memory.overview());
  const [selectedDocId, setSelectedDocId] = useState<string>('PROJECT_MEMORY.md');
  const [initConfirm, setInitConfirm] = useState(false);
  const [overwriteInit, setOverwriteInit] = useState(false);

  return (
    <>
      <PageHeader
        title={t('nav.memory')}
        description={t('memory.desc')}
        actions={
          <>
            <Button size="sm" onClick={() => void overview.reload()} loading={overview.loading}>
              {t('common.reload')}
            </Button>
            <HostOnly>
              <Button size="sm" variant="secondary" onClick={() => setInitConfirm(true)}>
                {t('memory.initBtn')}
              </Button>
            </HostOnly>
          </>
        }
      />

      <AsyncView state={overview}>
        {(ov: MemoryOverview) => (
          <div className="flex flex-col gap-5">
            <Card title={t('memory.workspaceTitle')} description={t('memory.redactNotice')}>
              <p className="text-sm">
                {t('skills.path')}: <Code>{ov.workspaceDir}</Code>
              </p>
              <QuickActionsBar onSaved={() => void overview.reload()} />
            </Card>

            <div className="grid gap-4 lg:grid-cols-[20rem_1fr]">
              <Card title={t('memory.filesTitle')}>
                <ul className="flex flex-col gap-1.5 text-sm" data-testid="memory-docs-list">
                  {ov.docs.map((d: MemoryDocStatus) => {
                    const active = d.id === selectedDocId;
                    return (
                      <li key={d.id}>
                        <button
                          type="button"
                          onClick={() => setSelectedDocId(d.id)}
                          className={`w-full rounded-md border p-2.5 text-left transition-colors ${
                            active ? 'border-primary bg-info-bg' : 'border-line hover:bg-surface-2'
                          }`}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="font-mono text-xs font-bold">{d.id}</span>
                            <Badge tone={d.exists ? 'success' : 'neutral'}>{d.exists ? formatBytes(d.sizeBytes) : t('memory.missing')}</Badge>
                          </div>
                          <p className="mt-1 text-xs text-muted">{lang === 'en' ? d.titleEn : d.titleVi}</p>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </Card>

              <DocEditorCard docId={selectedDocId} onSaved={() => void overview.reload()} />
            </div>

            <Card title={t('memory.handoffLogTitle')}>
              {ov.recentHandoffs.length === 0 ? (
                <p className="text-sm text-muted">{t('memory.noHandoffs')}</p>
              ) : (
                <ul className="divide-y divide-line text-sm">
                  {ov.recentHandoffs.map((h) => (
                    <li key={h.id} className="flex flex-wrap items-start justify-between gap-2 py-2">
                      <div className="min-w-0 flex-1">
                        <Badge tone="info">{h.agent}</Badge> <span className="ml-1">{h.summary}</span>
                      </div>
                      <span className="text-xs text-muted">{formatDate(h.createdAt, lang)}</span>
                    </li>
                  ))}
                </ul>
              )}
            </Card>
          </div>
        )}
      </AsyncView>

      <ConfirmDialog
        open={initConfirm}
        title={t('memory.initTitle')}
        confirmLabel={t('memory.initBtn')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setInitConfirm(false)}
        onConfirm={async () => {
          const r = await api.memory.init(overwriteInit);
          setInitConfirm(false);
          setOverwriteInit(false);
          notify.ok(t('memory.initDone', { n: r.created.length }));
          await overview.reload();
        }}
      >
        <p className="text-sm">{t('memory.initBody')}</p>
        <div className="mt-3">
          <Checkbox label={t('memory.overwriteExisting')} checked={overwriteInit} onChange={(e) => setOverwriteInit(e.target.checked)} />
        </div>
      </ConfirmDialog>
    </>
  );
}

function QuickActionsBar({ onSaved }: { onSaved: () => void }) {
  const { t } = useI18n();
  const notify = useNotify();
  const [agent, setAgent] = useState<AiProviderId>('antigravity');
  const [summary, setSummary] = useState('');
  const [nextAction, setNextAction] = useState('');
  const [busy, setBusy] = useState(false);

  return (
    <form
      className="mt-3 grid gap-2 rounded-md border border-line bg-surface-2/40 p-3 sm:grid-cols-[10rem_1fr_1fr_auto] sm:items-end"
      onSubmit={async (e) => {
        e.preventDefault();
        if (!summary.trim()) return;
        setBusy(true);
        try {
          await api.memory.recordHandoff({
            agent,
            summary: summary.trim(),
            nextAction: nextAction.trim() || undefined,
          });
          setSummary('');
          setNextAction('');
          notify.ok(t('memory.handoffSaved'));
          onSaved();
        } catch (ex) {
          notify.fail(ex);
        } finally {
          setBusy(false);
        }
      }}
    >
      <Select
        label="Agent"
        value={agent}
        onChange={(e) => setAgent(e.target.value as AiProviderId)}
        options={[
          { value: 'antigravity', label: 'Antigravity' },
          { value: 'claude', label: 'Claude Code' },
          { value: 'codex', label: 'OpenAI Codex' },
        ]}
      />
      <TextInput label={t('memory.summaryLabel')} placeholder={t('memory.summaryPlaceholder')} value={summary} onChange={(e) => setSummary(e.target.value)} />
      <TextInput label={t('memory.nextLabel')} placeholder="NEXT_ACTION…" value={nextAction} onChange={(e) => setNextAction(e.target.value)} />
      <Button type="submit" variant="primary" loading={busy} disabled={!summary.trim()}>
        {t('memory.recordBtn')}
      </Button>
    </form>
  );
}

function DocEditorCard({ docId, onSaved }: { docId: string; onSaved: () => void }) {
  const { t } = useI18n();
  const notify = useNotify();
  const doc = useApi(`memory-doc:${docId}`, () => api.memory.readDoc(docId));
  const [text, setText] = useState('');
  const [dirty, setDirty] = useState(false);
  const [confirmSave, setConfirmSave] = useState(false);

  useEffect(() => {
    if (doc.data?.id === docId) {
      setText(doc.data.content);
      setDirty(false);
    }
  }, [doc.data, docId]);

  return (
    <Card
      title={<span className="font-mono text-sm">{docId}</span>}
      description={doc.data ? <Code>{doc.data.relPath}</Code> : undefined}
      actions={
        <HostOnly>
          <Button size="sm" variant="primary" disabled={!dirty} onClick={() => setConfirmSave(true)} data-testid="memory-save-btn">
            {t('common.save')}
          </Button>
        </HostOnly>
      }
    >
      <AsyncView state={doc}>
        {() => (
          <TextArea
            label={docId}
            rows={18}
            className="font-mono text-xs"
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setDirty(true);
            }}
            spellCheck={false}
          />
        )}
      </AsyncView>
      <ConfirmDialog
        open={confirmSave}
        title={t('memory.saveConfirmTitle', { file: docId })}
        confirmLabel={t('common.save')}
        cancelLabel={t('common.cancel')}
        onCancel={() => setConfirmSave(false)}
        onConfirm={async () => {
          await api.memory.writeDoc(docId, text);
          setConfirmSave(false);
          setDirty(false);
          notify.ok(t('common.saved'), docId);
          await doc.reload();
          onSaved();
        }}
      >
        <p className="text-sm">{t('memory.redactNotice')}</p>
      </ConfirmDialog>
    </Card>
  );
}
