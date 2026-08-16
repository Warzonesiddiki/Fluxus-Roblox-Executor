import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';

// ─── WebSocket IPC client ──────────────────────────────────────────────────
// Connects to the Rust IPC bridge (ws://127.0.0.1:9090).

interface ExecResult {
  id: string;
  success: boolean;
  output: string;
  error?: string;
  duration_ms: number;
}

function useIpc() {
  const [connected, setConnected] = useState(false);
  const [consoleLines, setConsoleLines] = useState<string[]>([]);
  const [socket, setSocket] = useState<WebSocket | null>(null);

  useEffect(() => {
    const ws = new WebSocket('ws://127.0.0.1:9090');
    ws.onopen = () => {
      setConnected(true);
      setConsoleLines((prev) => [...prev, '[IPC] Bridge connected']);
    };
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data);
        if (msg.type === 'result') {
          const r = msg as ExecResult;
          setConsoleLines((prev) => [
            ...prev,
            `[${r.success ? 'OK' : 'ERR'}] ${r.output || r.error || ''} (${r.duration_ms}ms)`,
          ]);
        } else if (msg.type === 'status') {
          setConsoleLines((prev) => [...prev, `[SYS] ${msg.message}`]);
        } else if (msg.type === 'status_snapshot') {
          setStatus(msg.payload);
        } else if (msg.type === 'hub_list') {
          setHubScripts(msg.scripts ?? []);
        }
      } catch {
        setConsoleLines((prev) => [...prev, `[RAW] ${ev.data}`]);
      }
    };
    ws.onclose = () => {
      setConnected(false);
      setConsoleLines((prev) => [...prev, '[IPC] Bridge disconnected']);
    };
    setSocket(ws);
    return () => ws.close();
  }, []);

  const send = (obj: unknown) => {
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(obj));
    }
  };

  return { connected, consoleLines, send, setStatus, setHubScripts };
}

// ─── Feature toggles (Pepsi Matrix) ──────────────────────────────────────

const FEATURES = [
  ['token_farm', 'Token Farm'],
  ['auto_ability', 'Ability Tokens'],
  ['auto_link', 'Link Tokens'],
  ['auto_sprout', 'Sprouts'],
  ['auto_feed', 'Auto Feed Bees'],
  ['auto_train', 'Auto Train'],
  ['auto_blender', 'Blender Crafting'],
  ['auto_dispense', 'Dispenser Collection'],
  ['auto_quest', 'Quest Claims'],
  ['auto_cannon', 'Cannon Firing'],
  ['request_honeystorm', 'Honeystorm'],
  ['auto_meteor', 'Meteor Events'],
] as const;

function App() {
  const { connected, consoleLines, send, setStatus, setHubScripts } = useIpc();
  const [script, setScript] = useState(`-- WebSocket X v2
-- Sandboxed Luau execution test
local sum = 0
for i = 1, 100 do
  sum = sum + i
end
print("Sum of 1..100 =", sum)`);
  const [toggles, setToggles] = useState<Record<string, boolean>>(
    Object.fromEntries(FEATURES.map(([k]) => [k, true]))
  );
  const [status, setStatus] = useState<any>(null);
  const [hubScripts, setHubScripts] = useState<any[]>([]);

  const runScript = () => send({ type: 'execute', script, id: crypto.randomUUID() });

  const toggleFeature = (key: string, value: boolean) => {
    setToggles((prev) => ({ ...prev, [key]: value }));
    send({ type: 'configure', [key]: value });
  };

  return (
    <div style={{ display: 'flex', height: '100vh', flexDirection: 'column' }}>
      {/* Header */}
      <div style={{ padding: '12px 20px', background: '#151a22', display: 'flex', alignItems: 'center', gap: 12 }}>
        <span style={{ fontWeight: 700, fontSize: 18 }}>⚡ WebSocket X <span style={{ color: '#6cf' }}>v2</span></span>
        <span style={{
          padding: '3px 10px', borderRadius: 12, fontSize: 12,
          background: connected ? '#1d5c2e' : '#5c1d1d',
          color: connected ? '#7fe39a' : '#ff9a9a',
        }}>
          {connected ? '● Bridge Online' : '○ Bridge Offline'}
        </span>
        <button onClick={runScript} style={{
          marginLeft: 'auto', background: '#2563eb', color: '#fff', border: 'none',
          padding: '8px 22px', borderRadius: 8, fontSize: 14, cursor: 'pointer', fontWeight: 600,
        }}>▶ Execute</button>
      </div>

      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Left: script editor */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', padding: 12 }}>
          <div style={{ marginBottom: 8, color: '#999', fontSize: 12 }}>LUA SCRIPT</div>
          <textarea
            value={script}
            onChange={(e) => setScript(e.target.value)}
            spellCheck={false}
            style={{
              flex: 1, background: '#0d1016', color: '#d7dae0', border: '1px solid #2a3140',
              borderRadius: 8, padding: 14, fontFamily: 'ui-monospace, monospace', fontSize: 13,
              resize: 'none', outline: 'none',
            }}
          />
        </div>

        {/* Right: mode + status + features + hub + console */}
        <div style={{ width: 400, display: 'flex', flexDirection: 'column', gap: 12, padding: 12, borderLeft: '1px solid #1e2430', overflowY: 'auto' }}>
          {/* Mode switcher */}
          <div style={{ color: '#999', fontSize: 12 }}>PRODUCT MODE</div>
          <div style={{ display: 'flex', gap: 8 }}>
            {['external', 'internal', 'dev'].map((m) => (
              <button
                key={m}
                onClick={() => send({ type: 'set_mode', mode: m })}
                style={{
                  flex: 1, padding: '8px 0', borderRadius: 8, cursor: 'pointer',
                  background: status?.mode === m ? '#2563eb' : '#161a22',
                  color: status?.mode === m ? '#fff' : '#9fb4c7',
                  border: status?.mode === m ? '1px solid #2563eb' : '1px solid #2a3140',
                  textTransform: 'capitalize', fontSize: 12,
                }}
              >
                {m === 'external' ? '🛡 External' : m === 'internal' ? '⚡ Internal' : '🔧 Dev'}
              </button>
            ))}
          </div>

          {/* Status snapshot */}
          <div style={{ color: '#999', fontSize: 12 }}>ENGINE STATUS
            <button
              onClick={() => send({ type: 'get_status', id: crypto.randomUUID() })}
              style={{ marginLeft: 8, background: 'none', border: '1px solid #2a3140', color: '#9fb4c7', borderRadius: 6, padding: '2px 8px', cursor: 'pointer', fontSize: 11 }}
            >refresh</button>
          </div>
          {status && (
            <div style={{ background: '#0d1016', border: '1px solid #2a3140', borderRadius: 8, padding: 10, fontSize: 12 }}>
              <div>Mode: <b style={{ textTransform: 'capitalize' }}>{status.mode}</b></div>
              <div>State: <b>{status.behavior_state}</b></div>
              <div>Decision: <b>{status.decision}</b></div>
              <div>Ticks: <b>{status.tick_count}</b> · FPS: <b>{status.fps?.toFixed(1)}</b></div>
              <div>Confidence: <b>{status.fused?.confidence?.toFixed(2) ?? '—'}</b></div>
              <div>Position: <b>{status.fused ? `(${status.fused.position?.[0]?.toFixed(2)}, ${status.fused.position?.[1]?.toFixed(2)})` : '—'}</b></div>
            </div>
          )}

          {/* Script hub */}
          <div style={{ color: '#999', fontSize: 12 }}>SCRIPT HUB
            <button
              onClick={() => send({ type: 'hub_list', id: crypto.randomUUID() })}
              style={{ marginLeft: 8, background: 'none', border: '1px solid #2a3140', color: '#9fb4c7', borderRadius: 6, padding: '2px 8px', cursor: 'pointer', fontSize: 11 }}
            >load</button>
          </div>
          {hubScripts.length > 0 && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {hubScripts.map((s: any) => (
                <div key={s.id} style={{ background: '#0d1016', border: '1px solid #2a3140', borderRadius: 8, padding: 10 }}>
                  <div style={{ fontSize: 12, fontWeight: 600 }}>{s.name} <span style={{ color: '#6cf', fontSize: 10 }}>{s.mode}</span></div>
                  <div style={{ fontSize: 10, color: '#7a8699', margin: '4px 0' }}>{s.description}</div>
                  <button
                    onClick={() => send({ type: 'run_hub_script', id: s.id })}
                    style={{ background: '#1d5c2e', color: '#7fe39a', border: 'none', borderRadius: 6, padding: '4px 12px', cursor: 'pointer', fontSize: 11 }}
                  >▶ Run</button>
                </div>
              ))}
            </div>
          )}

          <div style={{ color: '#999', fontSize: 12 }}>FEATURE MATRIX (Pepsi Swarm)</div>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
            {FEATURES.map(([key, label]) => (
              <label key={key} style={{
                display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px',
                background: toggles[key] ? '#17263c' : '#161a22',
                borderRadius: 8, fontSize: 12, cursor: 'pointer',
                border: toggles[key] ? '1px solid #2563eb' : '1px solid #2a3140',
              }}>
                <input
                  type="checkbox"
                  checked={toggles[key]}
                  onChange={(e) => toggleFeature(key, e.target.checked)}
                />
                {label}
              </label>
            ))}
          </div>

          <div style={{ color: '#999', fontSize: 12, marginTop: 8 }}>CONSOLE OUTPUT</div>
          <div style={{
            flex: 1, background: '#0d1016', border: '1px solid #2a3140', borderRadius: 8,
            padding: 10, fontFamily: 'ui-monospace, monospace', fontSize: 11,
            overflowY: 'auto', color: '#9fb4c7',
          }}>
            {consoleLines.slice(-100).map((line, i) => (
              <div key={i}>{line}</div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
