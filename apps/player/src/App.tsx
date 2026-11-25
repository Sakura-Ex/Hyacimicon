import { useEffect, useState } from 'react';
import { DesignSystemView } from './views/DesignSystemView';
import { LiveDataArchiveView } from './views/LiveDataArchiveView';
import { LiveMatchView } from './views/LiveMatchView';
import { ModalGalleryView } from './views/ModalGalleryView';

type WorkspaceTab = 'hud' | 'modals' | 'design-system' | 'archive';

interface BridgeStatus {
  available: boolean;
  dataDir: string;
  errorMessage?: string;
}

const WORKSPACE_TABS: readonly { id: WorkspaceTab; label: string }[] = [
  { id: 'hud', label: '动态运营模拟' },
  { id: 'modals', label: '选择弹窗样板' },
  { id: 'design-system', label: '视觉规范与组件库' },
  { id: 'archive', label: '真实资产图鉴' },
];

export function App() {
  const [activeTab, setActiveTab] = useState<WorkspaceTab>('hud');
  const [bridgeStatus, setBridgeStatus] = useState<BridgeStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetch('/hsr-meta/status')
      .then((res) => {
        if (!res.ok) {
          throw new Error(`HTTP ${res.status}`);
        }
        return res.json();
      })
      .then((payload: unknown) => {
        if (
          !cancelled &&
          typeof payload === 'object' &&
          payload !== null &&
          'available' in payload &&
          typeof payload.available === 'boolean'
        ) {
          const dir =
            'dataDir' in payload && typeof payload.dataDir === 'string'
              ? payload.dataDir
              : '';
          setBridgeStatus({ available: payload.available, dataDir: dir });
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setBridgeStatus({
            available: false,
            dataDir: '',
            errorMessage: err instanceof Error ? err.message : '连接状态检测失败',
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="hsr-shell">
      <header className="hsr-shell-bar">
        <div className="hsr-shell-brand">
          <span className="hsr-shell-star" aria-hidden="true">
            ✦
          </span>
          <div className="hsr-shell-brand-text">
            <strong>HYACIMICON // CURRENCY WARS</strong>
            <span>崩坏：星穹铁道 · 货币战争前端视觉样板</span>
          </div>
        </div>

        <nav className="hsr-shell-tabs" aria-label="样板视图切换">
          {WORKSPACE_TABS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              className={`hsr-shell-tab ${activeTab === tab.id ? 'is-active' : ''}`}
              onClick={() => setActiveTab(tab.id)}
              aria-current={activeTab === tab.id ? 'page' : undefined}
            >
              {tab.label}
            </button>
          ))}
        </nav>

        <div
          className="hsr-bridge-pill"
          title={
            bridgeStatus?.errorMessage ??
            (bridgeStatus?.dataDir || '正在检测数据桥接')
          }
        >
          <span
            className={`hsr-bridge-dot ${bridgeStatus?.available ? '' : 'hsr-bridge-dot--offline'}`}
          />
          <span>
            {bridgeStatus === null
              ? '检测资产桥接中...'
              : bridgeStatus.available
                ? '数据源已连接 · hsr-currency-wars-data'
                : `数据源未连接${bridgeStatus.errorMessage ? ` (${bridgeStatus.errorMessage})` : ''}`}
          </span>
        </div>
      </header>

      <main className="hsr-shell-main">
        {activeTab === 'hud' && <LiveMatchView />}
        {activeTab === 'modals' && <ModalGalleryView />}
        {activeTab === 'design-system' && <DesignSystemView />}
        {activeTab === 'archive' && <LiveDataArchiveView />}
      </main>
    </div>
  );
}
export default App;
