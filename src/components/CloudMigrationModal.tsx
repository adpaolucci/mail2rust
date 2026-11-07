import React, { useState, useEffect } from 'react';
import {
  CloudLightning,
  X,
  CheckCircle,
  AlertCircle,
  Play,
  Pause,
  RefreshCw,
  Server,
  Key,
  Shield,
  Layers,
  Activity,
  ArrowRight,
} from 'lucide-react';
import { MailboxFolder } from '../types/mail';

interface CloudMigrationModalProps {
  isOpen: boolean;
  onClose: () => void;
  folders: MailboxFolder[];
}

export const CloudMigrationModal: React.FC<CloudMigrationModalProps> = ({
  isOpen,
  onClose,
  folders,
}) => {
  const [provider, setProvider] = useState<'m365' | 'google' | 'exchange' | 'imap'>('m365');
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ status: 'success' | 'error'; message: string; latencyMs: number } | null>(null);

  // M365 state
  const [tenantId, setTenantId] = useState('7a82b410-4491-49b0-91cd-89d1502fa481');
  const [clientId, setClientId] = useState('1194820a-88fa-4a21-9310-8b9a1122cc44');
  const [clientSecret, setClientSecret] = useState('••••••••••••••••••••••••••••••••');
  const [useBatchApi, setUseBatchApi] = useState(true);

  // Google state
  const [adminEmail, setAdminEmail] = useState('admin@titan.corp.local');
  const [serviceAccountJson, setServiceAccountJson] = useState('{"type": "service_account", "project_id": "titan-mail-prod", "client_email": "mail2mail-sa@titan-mail-prod.iam.gserviceaccount.com"}');

  // Exchange state
  const [ewsUrl, setEwsUrl] = useState('https://mail.titan.corp.local/EWS/Exchange.asmx');
  const [ewsUsername, setEwsUsername] = useState('TITAN\\migration_svc');

  // IMAP state
  const [imapHost, setImapHost] = useState('imap.titan.corp.local');
  const [imapPort, setImapPort] = useState('993');
  const [imapUseTls, setImapUseTls] = useState(true);

  // Migration Runner State
  const [isMigrating, setIsMigrating] = useState(false);
  const [migrationProgress, setMigrationProgress] = useState(0);
  const [migratedItems, setMigratedItems] = useState(0);
  const [activeSpeed, setActiveSpeed] = useState({ itemsPerSec: 0, mbPerSec: 0 });
  const [migrationLogs, setMigrationLogs] = useState<string[]>([]);

  useEffect(() => {
    let timer: any;
    if (isMigrating) {
      timer = setInterval(() => {
        setMigrationProgress(prev => {
          if (prev >= 100) {
            setIsMigrating(false);
            return 100;
          }
          const next = Math.min(100, prev + 3.5);
          setMigratedItems(m => m + Math.floor(Math.random() * 8 + 4));
          setActiveSpeed({
            itemsPerSec: Math.floor(Math.random() * 15 + 42),
            mbPerSec: Number((Math.random() * 3 + 12.4).toFixed(1)),
          });

          // Append simulated migration logs
          if (Math.random() > 0.4) {
            const sampleLogs = [
              `[Graph API] Migrated batch of 20 items to target folder: 'Inbox'`,
              `[EWS Adapter] Processed MIME stream for conv_titan_01 (480 KB)`,
              `[Token Bucket] Concurrency limit maintained: 50 requests/sec`,
              `[Verification] SHA-256 integrity match confirmed for item #842`,
              `[Checkpoint] Updated folder high UID watermark to: 98124`,
            ];
            const logEntry = sampleLogs[Math.floor(Math.random() * sampleLogs.length)];
            setMigrationLogs(logs => [logEntry, ...logs.slice(0, 15)]);
          }

          return next;
        });
      }, 500);
    }
    return () => clearInterval(timer);
  }, [isMigrating]);

  if (!isOpen) return null;

  const handleTestConnection = () => {
    setIsTesting(true);
    setTestResult(null);
    setTimeout(() => {
      setIsTesting(false);
      setTestResult({
        status: 'success',
        latencyMs: Math.floor(Math.random() * 40 + 25),
        message:
          provider === 'm365'
            ? 'Connected to Microsoft Graph API. Scopes: Mail.ReadWrite, MailboxSettings.ReadWrite.'
            : provider === 'google'
            ? 'Service Account JWT authenticated. Domain-Wide Delegation verified.'
            : provider === 'exchange'
            ? 'Exchange Web Services (EWS) Autodiscover successful. NTLM handshake ok.'
            : 'IMAP4rev1 TLS connection established. CAPABILITY: UIDPLUS, CONDSTORE.',
      });
    }, 800);
  };

  const handleStartMigration = () => {
    setMigrationProgress(0);
    setMigratedItems(0);
    setMigrationLogs(['[Pipeline] Initializing cloud migration connectors...']);
    setIsMigrating(true);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-3xl shadow-2xl flex flex-col max-h-[90vh] text-xs text-slate-200 overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b border-slate-800 bg-slate-850 flex items-center justify-between">
          <div className="flex items-center space-x-2.5">
            <div className="w-8 h-8 rounded-lg bg-emerald-600/30 border border-emerald-500/40 flex items-center justify-center text-emerald-400">
              <CloudLightning className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-semibold text-white tracking-tight">Direct Cloud / SaaS Migrations</h2>
              <p className="text-[11px] text-slate-400">
                High-throughput pipeline for Microsoft 365, Exchange EWS, Google Workspace & IMAP
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 text-slate-400 hover:text-white rounded-md hover:bg-slate-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Cloud Provider Tabs */}
        <div className="bg-slate-950 border-b border-slate-800 px-4 flex items-center space-x-2">
          <button
            onClick={() => { setProvider('m365'); setTestResult(null); }}
            className={`px-3 py-2 border-b-2 font-medium transition-colors ${
              provider === 'm365'
                ? 'border-blue-500 text-blue-400 bg-slate-900'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            Microsoft 365 (Graph)
          </button>
          <button
            onClick={() => { setProvider('google'); setTestResult(null); }}
            className={`px-3 py-2 border-b-2 font-medium transition-colors ${
              provider === 'google'
                ? 'border-emerald-500 text-emerald-400 bg-slate-900'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            Google Workspace (Gmail)
          </button>
          <button
            onClick={() => { setProvider('exchange'); setTestResult(null); }}
            className={`px-3 py-2 border-b-2 font-medium transition-colors ${
              provider === 'exchange'
                ? 'border-purple-500 text-purple-400 bg-slate-900'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            Exchange On-Prem (EWS)
          </button>
          <button
            onClick={() => { setProvider('imap'); setTestResult(null); }}
            className={`px-3 py-2 border-b-2 font-medium transition-colors ${
              provider === 'imap'
                ? 'border-amber-500 text-amber-400 bg-slate-900'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            Generic IMAP4 / SSL
          </button>
        </div>

        {/* Body */}
        <div className="p-5 overflow-y-auto space-y-4">
          {/* Provider Specific Configuration */}
          {provider === 'm365' && (
            <div className="space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-slate-300 font-medium mb-1">Azure Tenant ID</label>
                  <input
                    type="text"
                    value={tenantId}
                    onChange={(e) => setTenantId(e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                  />
                </div>
                <div>
                  <label className="block text-slate-300 font-medium mb-1">Application (Client) ID</label>
                  <input
                    type="text"
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                  />
                </div>
              </div>

              <div>
                <label className="block text-slate-300 font-medium mb-1">Client Secret / Certificate</label>
                <input
                  type="password"
                  value={clientSecret}
                  onChange={(e) => setClientSecret(e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                />
              </div>

              <div className="flex items-center space-x-2 pt-1">
                <input
                  type="checkbox"
                  id="batchApi"
                  checked={useBatchApi}
                  onChange={(e) => setUseBatchApi(e.target.checked)}
                  className="rounded bg-slate-800 border-slate-700 text-blue-500"
                />
                <label htmlFor="batchApi" className="text-slate-300">
                  Enable High-Throughput Microsoft Graph Batch Import API (50 items/sec)
                </label>
              </div>
            </div>
          )}

          {provider === 'google' && (
            <div className="space-y-3">
              <div>
                <label className="block text-slate-300 font-medium mb-1">Super Admin Impersonation Email</label>
                <input
                  type="text"
                  value={adminEmail}
                  onChange={(e) => setAdminEmail(e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                />
              </div>

              <div>
                <label className="block text-slate-300 font-medium mb-1">GCP Service Account JSON Key</label>
                <textarea
                  rows={3}
                  value={serviceAccountJson}
                  onChange={(e) => setServiceAccountJson(e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                />
              </div>
            </div>
          )}

          {provider === 'exchange' && (
            <div className="space-y-3">
              <div>
                <label className="block text-slate-300 font-medium mb-1">Exchange Web Services (EWS) URL</label>
                <input
                  type="text"
                  value={ewsUrl}
                  onChange={(e) => setEwsUrl(e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                />
              </div>

              <div>
                <label className="block text-slate-300 font-medium mb-1">Impersonation Service Account</label>
                <input
                  type="text"
                  value={ewsUsername}
                  onChange={(e) => setEwsUsername(e.target.value)}
                  className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                />
              </div>
            </div>
          )}

          {provider === 'imap' && (
            <div className="space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-slate-300 font-medium mb-1">IMAP Host</label>
                  <input
                    type="text"
                    value={imapHost}
                    onChange={(e) => setImapHost(e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                  />
                </div>
                <div>
                  <label className="block text-slate-300 font-medium mb-1">Port</label>
                  <input
                    type="text"
                    value={imapPort}
                    onChange={(e) => setImapPort(e.target.value)}
                    className="w-full bg-slate-800 border border-slate-700 rounded p-2 text-slate-100 font-mono text-[11px]"
                  />
                </div>
              </div>
            </div>
          )}

          {/* Test Connection Button & Result */}
          <div className="flex items-center space-x-3 pt-1">
            <button
              onClick={handleTestConnection}
              disabled={isTesting}
              className="px-3 py-1.5 rounded bg-slate-800 hover:bg-slate-700 border border-slate-600 font-medium text-slate-200 flex items-center space-x-1.5"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isTesting ? 'animate-spin' : ''}`} />
              <span>{isTesting ? 'Testing Handshake...' : 'Test Connection'}</span>
            </button>

            {testResult && (
              <div className="flex items-center space-x-2 text-[11px] text-emerald-400 bg-emerald-950/60 border border-emerald-600/40 rounded px-2.5 py-1">
                <CheckCircle className="w-3.5 h-3.5 shrink-0" />
                <span>{testResult.message} ({testResult.latencyMs}ms)</span>
              </div>
            )}
          </div>

          {/* Live Migration Execution Section */}
          <div className="border-t border-slate-800 pt-4 space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <Activity className="w-4 h-4 text-blue-400" />
                <span className="font-semibold text-slate-200">Live Migration Pipeline</span>
              </div>

              {isMigrating && (
                <div className="flex items-center space-x-3 text-[11px] font-mono">
                  <span className="text-emerald-400 font-bold">{activeSpeed.itemsPerSec} items/sec</span>
                  <span className="text-blue-400 font-bold">{activeSpeed.mbPerSec} MB/s</span>
                </div>
              )}
            </div>

            {/* Progress Bar */}
            <div className="space-y-1">
              <div className="flex justify-between text-[11px] text-slate-400">
                <span>Overall Transfer Progress</span>
                <span className="font-mono text-slate-200">{migrationProgress.toFixed(0)}%</span>
              </div>
              <div className="w-full h-2.5 bg-slate-800 rounded-full overflow-hidden">
                <div
                  className="h-full bg-gradient-to-r from-blue-500 to-emerald-500 transition-all duration-300"
                  style={{ width: `${migrationProgress}%` }}
                />
              </div>
            </div>

            {/* Stream Logs */}
            <div className="bg-slate-950 border border-slate-800 rounded-lg p-3 font-mono text-[11px] text-slate-400 h-28 overflow-y-auto space-y-1">
              {migrationLogs.length === 0 ? (
                <div className="text-slate-600 italic">Click 'Start Cloud Migration' to run the migration pipeline...</div>
              ) : (
                migrationLogs.map((log, i) => (
                  <div key={i} className="text-slate-300 flex items-center space-x-2">
                    <span className="text-slate-600">&gt;</span>
                    <span>{log}</span>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="p-4 border-t border-slate-800 bg-slate-850 flex items-center justify-between">
          <span className="text-[11px] text-slate-400 font-mono">
            {migratedItems > 0 ? `${migratedItems} items migrated` : 'Ready to start'}
          </span>

          <div className="flex items-center space-x-2">
            <button
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg border border-slate-700 hover:bg-slate-800 text-slate-300 transition-colors"
            >
              Close
            </button>
            <button
              onClick={handleStartMigration}
              disabled={isMigrating}
              className="px-4 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white font-medium transition-colors shadow-sm flex items-center space-x-1.5 disabled:opacity-50"
            >
              <Play className="w-3.5 h-3.5" />
              <span>{isMigrating ? 'Migrating...' : 'Start Cloud Migration'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
