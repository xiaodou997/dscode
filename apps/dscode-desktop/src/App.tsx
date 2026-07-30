import { useEffect, useState } from "react";
import {
  AlertCircle,
  ArrowUpRight,
  Check,
  CheckCircle2,
  ChevronRight,
  Code2,
  Eye,
  EyeOff,
  FileCode2,
  HardDrive,
  Image,
  KeyRound,
  LoaderCircle,
  MessageSquare,
  Play,
  RefreshCw,
  RotateCcw,
  Settings,
  ShieldCheck,
  Sparkles,
  TerminalSquare,
  X,
} from "lucide-react";
import {
  type DashboardState,
  configureDouStack,
  forgetDouStackKey,
  launchOfficialCodex,
  loadDashboard,
  openOfficialDownload,
  previewConfig,
  restoreLatestConfig,
} from "./api";

type Page = "code" | "settings";
type PendingAction = "configure" | "restore" | null;
type Notice = { tone: "success" | "error"; message: string } | null;

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function pathTail(path: string | null) {
  if (!path) return "尚无备份";
  const segments = path.split("/");
  return segments.at(-1) || path;
}

export default function App() {
  const [page, setPage] = useState<Page>("code");
  const [state, setState] = useState<DashboardState | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [pendingAction, setPendingAction] = useState<PendingAction>(null);
  const [notice, setNotice] = useState<Notice>(null);
  const [preview, setPreview] = useState<string | null>(null);

  async function refresh() {
    setBusy("refresh");
    try {
      setState(await loadDashboard());
      setNotice(null);
    } catch (error) {
      setNotice({ tone: "error", message: errorMessage(error) });
    } finally {
      setBusy(null);
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function confirmAction() {
    const action = pendingAction;
    setPendingAction(null);
    if (!action) return;
    setBusy(action);
    try {
      if (action === "configure") {
        const result = await configureDouStack(apiKey);
        setState(result.state);
        setApiKey("");
        setNotice({
          tone: "success",
          message: result.changed ? "DouStack 配置已应用并完成备份" : "当前已经是 DouStack 配置",
        });
      } else {
        const result = await restoreLatestConfig();
        setState(result.state);
        setNotice({ tone: "success", message: "最近一次官方配置已恢复" });
      }
    } catch (error) {
      setNotice({ tone: "error", message: errorMessage(error) });
    } finally {
      setBusy(null);
    }
  }

  async function showPreview() {
    setBusy("preview");
    try {
      setPreview(await previewConfig());
    } catch (error) {
      setNotice({ tone: "error", message: errorMessage(error) });
    } finally {
      setBusy(null);
    }
  }

  async function launch() {
    setBusy("launch");
    try {
      await launchOfficialCodex();
      setNotice({ tone: "success", message: "官方 Codex 已启动" });
      window.setTimeout(() => void refresh(), 1200);
    } catch (error) {
      setNotice({ tone: "error", message: errorMessage(error) });
      setBusy(null);
    }
  }

  async function forgetKey() {
    setBusy("forget");
    try {
      setState(await forgetDouStackKey());
      setNotice({ tone: "success", message: "已移除保存的 DouStack Key" });
    } catch (error) {
      setNotice({ tone: "error", message: errorMessage(error) });
    } finally {
      setBusy(null);
    }
  }

  if (loading || !state) {
    return (
      <div className="loading-screen">
        <div className="brand-mark" aria-hidden="true">DS</div>
        <LoaderCircle className="spin" size={22} />
        <span>正在读取 Codex 状态</span>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand-block">
          <div className="brand-mark" aria-hidden="true">DS</div>
          <div>
            <strong>DS Code</strong>
            <span>DouStack</span>
          </div>
        </div>

        <nav className="primary-nav" aria-label="主导航">
          <button className={page === "code" ? "nav-item active" : "nav-item"} onClick={() => setPage("code")} aria-label="Code" title="Code">
            <Code2 size={18} />
            <span>Code</span>
          </button>
          <button className="nav-item" disabled title="等待 DouStack Chat 接口接入" aria-label="Chat（即将推出）">
            <MessageSquare size={18} />
            <span>Chat</span>
            <small>稍后</small>
          </button>
          <button className="nav-item" disabled title="等待 DouStack Images 接口接入" aria-label="Images（即将推出）">
            <Image size={18} />
            <span>Images</span>
            <small>稍后</small>
          </button>
        </nav>

        <div className="nav-spacer" />
        <button className={page === "settings" ? "nav-item active" : "nav-item"} onClick={() => setPage("settings")} aria-label="设置" title="设置">
          <Settings size={18} />
          <span>设置</span>
        </button>
        <div className="sidebar-status">
          <span className={state.provider.doustackActive ? "status-dot online" : "status-dot"} />
          <div>
            <strong>{state.provider.doustackActive ? "DouStack 已启用" : "官方配置"}</strong>
            <span>{state.officialApp.running ? "Codex 正在运行" : "Codex 已停止"}</span>
          </div>
        </div>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <div>
            <span className="context-label">DOUSTACK CODE</span>
            <h1>{page === "code" ? "Codex 工作台" : "设置"}</h1>
          </div>
          <button className="icon-button" onClick={() => void refresh()} disabled={busy !== null} title="刷新状态" aria-label="刷新状态">
            <RefreshCw size={18} className={busy === "refresh" ? "spin" : ""} />
          </button>
        </header>

        {notice && (
          <div className={`notice ${notice.tone}`} role="status">
            {notice.tone === "success" ? <CheckCircle2 size={18} /> : <AlertCircle size={18} />}
            <span>{notice.message}</span>
            <button onClick={() => setNotice(null)} aria-label="关闭通知"><X size={16} /></button>
          </div>
        )}

        {page === "code" ? (
          <CodeWorkspace
            state={state}
            apiKey={apiKey}
            showKey={showKey}
            busy={busy}
            onApiKeyChange={setApiKey}
            onToggleKey={() => setShowKey((visible) => !visible)}
            onConfigure={() => setPendingAction("configure")}
            onRestore={() => setPendingAction("restore")}
            onPreview={() => void showPreview()}
            onLaunch={() => void launch()}
            onDownload={() => void openOfficialDownload()}
          />
        ) : (
          <SettingsPage state={state} busy={busy} onForgetKey={() => void forgetKey()} />
        )}
      </main>

      {pendingAction && (
        <div className="modal-backdrop" role="presentation" onMouseDown={() => setPendingAction(null)}>
          <section className="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="confirm-title" onMouseDown={(event) => event.stopPropagation()}>
            <div className="dialog-icon"><ShieldCheck size={22} /></div>
            <h2 id="confirm-title">{pendingAction === "configure" ? "应用 DouStack 配置" : "恢复最近备份"}</h2>
            <p>
              {pendingAction === "configure"
                ? "DS Code 将备份并更新 ~/.codex/config.toml。官方登录和其他 Codex 设置会保留。"
                : "当前 config.toml 将替换为最近一次备份，官方 Codex 必须保持关闭。"}
            </p>
            <div className="dialog-actions">
              <button className="button secondary" onClick={() => setPendingAction(null)}>取消</button>
              <button className="button primary" onClick={() => void confirmAction()}>
                <Check size={17} />确认
              </button>
            </div>
          </section>
        </div>
      )}

      {preview !== null && (
        <div className="modal-backdrop" role="presentation" onMouseDown={() => setPreview(null)}>
          <section className="preview-dialog" role="dialog" aria-modal="true" aria-labelledby="preview-title" onMouseDown={(event) => event.stopPropagation()}>
            <div className="preview-header">
              <div>
                <span className="context-label">CONFIG PREVIEW</span>
                <h2 id="preview-title">配置预览</h2>
              </div>
              <button className="icon-button" onClick={() => setPreview(null)} aria-label="关闭预览"><X size={18} /></button>
            </div>
            <pre>{preview}</pre>
          </section>
        </div>
      )}
    </div>
  );
}

type CodeWorkspaceProps = {
  state: DashboardState;
  apiKey: string;
  showKey: boolean;
  busy: string | null;
  onApiKeyChange: (value: string) => void;
  onToggleKey: () => void;
  onConfigure: () => void;
  onRestore: () => void;
  onPreview: () => void;
  onLaunch: () => void;
  onDownload: () => void;
};

function CodeWorkspace({
  state,
  apiKey,
  showKey,
  busy,
  onApiKeyChange,
  onToggleKey,
  onConfigure,
  onRestore,
  onPreview,
  onLaunch,
  onDownload,
}: CodeWorkspaceProps) {
  const hasNewKey = apiKey.trim().length > 0;
  const canConfigure = (hasNewKey || state.credentialSaved)
    && (hasNewKey || !state.provider.doustackActive)
    && !state.officialApp.running
    && busy === null;
  const canLaunch = state.officialApp.installed && (!state.provider.doustackActive || state.credentialSaved) && busy === null;

  return (
    <div className="workspace-page">
      <section className="summary-strip" aria-label="运行状态">
        <SummaryMetric label="官方应用" value={state.officialApp.installed ? `已安装 ${state.officialApp.version ?? ""}`.trim() : "未安装"} good={state.officialApp.installed} />
        <SummaryMetric label="官方账号" value={state.officialAuthPresent ? "已检测" : "未检测"} good={state.officialAuthPresent} />
        <SummaryMetric label="当前通道" value={state.provider.doustackActive ? "DouStack" : state.provider.activeProvider ?? "OpenAI"} good={state.provider.doustackActive} />
        <SummaryMetric label="配置备份" value={pathTail(state.latestBackup)} good={state.latestBackup !== null} />
      </section>

      <div className="workspace-grid">
        <section className="runtime-section">
          <div className="section-heading">
            <div>
              <span className="context-label">OFFICIAL RUNTIME</span>
              <h2>官方 Codex</h2>
            </div>
            <StatusPill online={state.officialApp.running} label={state.officialApp.running ? "运行中" : "已停止"} />
          </div>

          <div className="detail-list">
            <DetailRow icon={<HardDrive size={18} />} label="应用位置" value={state.officialApp.path ?? "未检测到官方 Codex"} status={state.officialApp.installed ? "ok" : "warn"} />
            <DetailRow icon={<ShieldCheck size={18} />} label="官方登录" value={state.officialAuthPresent ? "登录状态文件已检测" : "未检测到官方登录状态"} status={state.officialAuthPresent ? "ok" : "neutral"} />
            <DetailRow icon={<FileCode2 size={18} />} label="Codex 数据" value={state.codexHome} status="ok" />
            <DetailRow icon={<Sparkles size={18} />} label="DouStack API" value={state.endpoint} status={state.provider.doustackActive ? "ok" : "neutral"} />
          </div>

          <div className="runtime-actions">
            {state.officialApp.installed ? (
              <button className="button primary" disabled={!canLaunch} onClick={onLaunch}>
                {busy === "launch" ? <LoaderCircle className="spin" size={17} /> : <Play size={17} />}
                {state.officialApp.running ? "打开 Codex" : "启动 Codex"}
              </button>
            ) : (
              <button className="button primary" onClick={onDownload}>
                <ArrowUpRight size={17} />安装官方 Codex
              </button>
            )}
            <button className="button secondary" onClick={onPreview} disabled={busy !== null}>
              <FileCode2 size={17} />预览配置
            </button>
          </div>
        </section>

        <aside className="provider-panel">
          <div className="section-heading compact">
            <div>
              <span className="context-label">PROVIDER</span>
              <h2>DouStack 接入</h2>
            </div>
            {state.provider.doustackActive && <CheckCircle2 className="success-icon" size={21} />}
          </div>

          <label className="field-label" htmlFor="api-key">API Key</label>
          <div className="secret-input">
            <KeyRound size={18} />
            <input
              id="api-key"
              type={showKey ? "text" : "password"}
              value={apiKey}
              onChange={(event) => onApiKeyChange(event.target.value)}
              placeholder={state.credentialSaved ? "已保存，输入新 Key 可替换" : "输入 DouStack API Key"}
              autoComplete="off"
              spellCheck={false}
            />
            <button onClick={onToggleKey} title={showKey ? "隐藏 Key" : "显示 Key"} aria-label={showKey ? "隐藏 Key" : "显示 Key"}>
              {showKey ? <EyeOff size={18} /> : <Eye size={18} />}
            </button>
          </div>
          <div className="field-meta">
            <span className={state.credentialSaved ? "saved" : ""}>{state.credentialSaved ? "Key 已保存" : "尚未保存 Key"}</span>
            <span>Responses API</span>
          </div>

          <button className="button primary full" disabled={!canConfigure} onClick={onConfigure}>
            {busy === "configure" ? <LoaderCircle className="spin" size={17} /> : <ShieldCheck size={17} />}
            备份并启用 DouStack
          </button>

          <div className="provider-state">
            <div>
              <span className={state.provider.doustackConfigured ? "state-icon active" : "state-icon"}>
                {state.provider.doustackConfigured ? <Check size={14} /> : "1"}
              </span>
              <div><strong>Provider 配置</strong><span>{state.provider.doustackConfigured ? "已写入" : "等待配置"}</span></div>
            </div>
            <ChevronRight size={16} />
            <div>
              <span className={state.provider.doustackActive ? "state-icon active" : "state-icon"}>
                {state.provider.doustackActive ? <Check size={14} /> : "2"}
              </span>
              <div><strong>请求通道</strong><span>{state.provider.doustackActive ? "DouStack" : "官方"}</span></div>
            </div>
          </div>

          <button className="text-button danger" disabled={!state.latestBackup || state.officialApp.running || busy !== null} onClick={onRestore}>
            <RotateCcw size={16} />恢复最近配置备份
          </button>
        </aside>
      </div>

      <section className="fallback-section">
        <div className="fallback-icon"><TerminalSquare size={21} /></div>
        <div className="fallback-copy">
          <span className="context-label">LOCAL FALLBACK</span>
          <h3>独立编码运行时</h3>
          <p>{state.localRuntime.available ? `本机 Codex CLI ${state.localRuntime.version ?? ""}` : "本机未安装 Codex CLI"}</p>
        </div>
        <div className="fallback-status">
          <StatusPill online={state.localRuntime.tested} label={state.localRuntime.tested ? "兼容性已验证" : `基线 ${state.localRuntime.testedVersion}`} />
        </div>
      </section>
    </div>
  );
}

function SettingsPage({ state, busy, onForgetKey }: { state: DashboardState; busy: string | null; onForgetKey: () => void }) {
  return (
    <div className="settings-page">
      <section className="settings-section">
        <div className="section-heading"><div><span className="context-label">PATHS</span><h2>数据与配置</h2></div></div>
        <div className="settings-rows">
          <div><span>Codex 数据目录</span><code>{state.codexHome}</code></div>
          <div><span>固定 API 地址</span><code>{state.endpoint}</code></div>
          <div><span>最近配置备份</span><code>{state.latestBackup ?? "尚无备份"}</code></div>
        </div>
      </section>
      <section className="settings-section">
        <div className="section-heading"><div><span className="context-label">CREDENTIAL</span><h2>DouStack Key</h2></div></div>
        <div className="credential-row">
          <div className={state.credentialSaved ? "credential-badge saved" : "credential-badge"}><KeyRound size={18} /></div>
          <div><strong>{state.credentialSaved ? "已保存" : "未保存"}</strong><span>{state.credentialError ?? "DS Code 不会在界面和日志中显示 Key"}</span></div>
          <button className="button secondary danger" disabled={!state.credentialSaved || busy !== null} onClick={onForgetKey}>移除 Key</button>
        </div>
      </section>
    </div>
  );
}

function SummaryMetric({ label, value, good }: { label: string; value: string; good: boolean }) {
  return <div className="summary-metric"><span>{label}</span><strong>{value}</strong><i className={good ? "metric-indicator good" : "metric-indicator"} /></div>;
}

function StatusPill({ online, label }: { online: boolean; label: string }) {
  return <span className={online ? "status-pill online" : "status-pill"}><i />{label}</span>;
}

function DetailRow({ icon, label, value, status }: { icon: React.ReactNode; label: string; value: string; status: "ok" | "warn" | "neutral" }) {
  return <div className="detail-row"><span className={`detail-icon ${status}`}>{icon}</span><div><span>{label}</span><strong title={value}>{value}</strong></div>{status === "ok" && <CheckCircle2 size={17} className="row-check" />}</div>;
}
