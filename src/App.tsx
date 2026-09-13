import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface Device {
  id: string;
  model: string;
  serial: string;
  capacity_bytes: number;
  media_type: string;
  bus_type: string;
  is_removable: boolean;
}

interface Identity {
  operator_id: string;
  fingerprint: string;
  report_dir: string;
}

interface StdInfo {
  id: string;
  label: string;
}

interface ProgressUpdate {
  operation_id: string;
  operation_type: string;
  phase: string;
  bytes_done: number;
  total_bytes: number;
  message: string;
}

interface OperationOutcome {
  operation_id: string;
  report_json: string;
  report_pdf: string;
}

interface ReportMeta {
  filename: string;
  report_id: string;
  operation_type: string;
  operator_id: string;
  target: string;
  start_time: string;
  standard_label: string;
  verified: boolean;
  has_pdf: boolean;
}

interface ReportView {
  report: {
    report_id: string;
    operation_type: string;
    operation_id: string;
    operator_id: string;
    target: string;
    standard_id: string;
    standard_label: string;
    capacity_bytes: number;
    start_time: string;
    finish_time: string;
    verification: {
      status: string;
      bytes_verified: number;
      mismatched_sectors: number;
      detail: string;
    };
    skipped_sector_count: number;
    report_hash: string;
    signature_alg: string;
    signature: string;
  };
  verified: boolean;
}

type Tab = "wipe" | "erase" | "carve" | "reports";

interface SyncStatus {
  enabled: boolean;
  configured: boolean;
  url: string;
  synced: number;
  total: number;
}

function formatBytes(bytes: number): string {
  if (!bytes) return "—";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value % 1 === 0 && unit === 0 ? 0 : 2)} ${units[unit]}`;
}

function shortTime(iso: string): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleString();
}

function App() {
  const [identity, setIdentity] = useState<Identity | null>(null);
  const [devices, setDevices] = useState<Device[]>([]);
  const [wipeStandards, setWipeStandards] = useState<StdInfo[]>([]);
  const [eraseStandards, setEraseStandards] = useState<StdInfo[]>([]);
  const [reports, setReports] = useState<ReportMeta[]>([]);
  const [active, setActive] = useState<Record<string, ProgressUpdate>>({});
  const [lastOutcome, setLastOutcome] = useState<OperationOutcome | null>(null);
  const [error, setError] = useState<string>("");
  const [tab, setTab] = useState<Tab>("wipe");

  const [operatorInput, setOperatorInput] = useState("");
  const [busy, setBusy] = useState<Record<string, boolean>>({});

  const [wipeTargetKind, setWipeTargetKind] = useState<"device" | "file">("file");
  const [wipePath, setWipePath] = useState("");
  const [wipeDevicePath, setWipeDevicePath] = useState("");
  const [wipeStandard, setWipeStandard] = useState("");
  const [wipeConfirm, setWipeConfirm] = useState("");

  const [eraseKind, setEraseKind] = useState<"file" | "folder">("file");
  const [erasePath, setErasePath] = useState("");
  const [eraseStandard, setEraseStandard] = useState("");
  const [eraseConfirm, setEraseConfirm] = useState("");

  const [carveSource, setCarveSource] = useState("");
  const [carveOutput, setCarveOutput] = useState("");

  const [selectedReport, setSelectedReport] = useState<ReportView | null>(null);
  const [selectedFilename, setSelectedFilename] = useState("");

  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [syncBusy, setSyncBusy] = useState(false);

  const refreshReports = useCallback(async () => {
    try {
      const list = await invoke<ReportMeta[]>("list_reports");
      setReports(list);
    } catch (e) {
      setError(`list_reports failed: ${e}`);
    }
  }, []);

  const refreshSync = useCallback(async () => {
    try {
      setSyncStatus(await invoke<SyncStatus>("get_sync_status"));
    } catch (e) {
      setError(`get_sync_status failed: ${e}`);
    }
  }, []);

  useEffect(() => {
    (async () => {
      try {
        const id = await invoke<Identity>("get_identity");
        setIdentity(id);
        setOperatorInput(id.operator_id);
      } catch (e) {
        setError(`get_identity failed: ${e}`);
      }
      try {
        setDevices(await invoke<Device[]>("list_devices"));
      } catch {
        /* device listing is best-effort */
      }
      try {
        setWipeStandards(await invoke<StdInfo[]>("get_wipe_standards"));
        setWipeStandard((await invoke<StdInfo[]>("get_wipe_standards"))[0]?.id ?? "");
      } catch (e) {
        setError(`standards failed: ${e}`);
      }
      try {
        setEraseStandards(await invoke<StdInfo[]>("get_erase_standards"));
        setEraseStandard((await invoke<StdInfo[]>("get_erase_standards"))[0]?.id ?? "");
      } catch (e) {
        setError(`standards failed: ${e}`);
      }
      await refreshReports();
      await refreshSync();
    })();
    return;
  }, [refreshReports, refreshSync]);

  useEffect(() => {
    let unlisteners: UnlistenFn[] = [];
    (async () => {
      unlisteners.push(
        await listen<ProgressUpdate>("progress", (event) => {
          const upd = event.payload;
          setActive((prev) => ({
            ...prev,
            [upd.operation_id]: upd,
          }));
          if (upd.phase === "done") {
            setActive((prev) => {
              const next = { ...prev };
              delete next[upd.operation_id];
              return next;
            });
          }
        })
      );
      unlisteners.push(
        await listen<OperationOutcome>("operation-complete", (event) => {
          setLastOutcome(event.payload);
          setBusy((prev) => ({ ...prev, [event.payload.operation_id]: false }));
          refreshReports();
        })
      );
      unlisteners.push(
        await listen<{ operation_id: string; error: string }>("operation-error", (event) => {
          setError(`operation ${event.payload.operation_id} failed: ${event.payload.error}`);
          setBusy((prev) => ({ ...prev, [event.payload.operation_id]: false }));
        })
      );
      unlisteners.push(
        await listen<void>("sync-state", () => {
          refreshSync();
        })
      );
    })();
    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [refreshReports, refreshSync]);

  const updateOperator = async () => {
    try {
      const next = await invoke<string>("set_operator", { operatorId: operatorInput });
      setIdentity((prev) => (prev ? { ...prev, operator_id: next } : prev));
    } catch (e) {
      setError(`set_operator failed: ${e}`);
    }
  };

  const selectedDevice = devices.find((d) => d.id === wipeDevicePath);

  const wipeExpected: string = useMemo(() => {
    if (wipeTargetKind === "file") return `image file ${wipePath}`;
    if (!selectedDevice) return "";
    return `device ${selectedDevice.id} (${selectedDevice.capacity_bytes} bytes)`;
  }, [wipeTargetKind, wipePath, selectedDevice]);

  const eraseExpected: string = `${eraseKind} ${erasePath}`;

  const wipeReady = wipeConfirm === wipeExpected && wipeStandard && (wipeTargetKind === "file" || !!selectedDevice);
  const eraseReady = eraseConfirm === eraseExpected && eraseStandard && erasePath;
  const carveReady = carveSource && carveOutput;

  const runWipe = async () => {
    if (!wipeReady || !wipeStandard) return;
    const opKey = `wipe-${Date.now()}`;
    setBusy((prev) => ({ ...prev, [opKey]: true }));
    setError("");
    try {
      if (wipeTargetKind === "file") {
        await invoke<string>("wipe_image", {
          path: wipePath,
          standardId: wipeStandard,
          confirmed: wipeConfirm,
        });
      } else if (selectedDevice) {
        await invoke<string>("wipe_device", {
          devicePath: selectedDevice.id,
          capacityBytes: selectedDevice.capacity_bytes,
          standardId: wipeStandard,
          confirmed: wipeConfirm,
        });
      }
    } catch (e) {
      setError(`wipe failed: ${e}`);
      setBusy((prev) => ({ ...prev, [opKey]: false }));
    }
  };

  const runErase = async () => {
    if (!eraseReady || !eraseStandard) return;
    const opKey = `erase-${Date.now()}`;
    setBusy((prev) => ({ ...prev, [opKey]: true }));
    setError("");
    try {
      await invoke<string>("erase_path", {
        targetType: eraseKind,
        path: erasePath,
        standardId: eraseStandard,
        confirmed: eraseConfirm,
      });
    } catch (e) {
      setError(`erase failed: ${e}`);
      setBusy((prev) => ({ ...prev, [opKey]: false }));
    }
  };

  const runCarve = async () => {
    if (!carveReady) return;
    const opKey = `carve-${Date.now()}`;
    setBusy((prev) => ({ ...prev, [opKey]: true }));
    setError("");
    try {
      await invoke<string>("carve_source", {
        source: carveSource,
        outputDir: carveOutput,
      });
    } catch (e) {
      setError(`carve failed: ${e}`);
      setBusy((prev) => ({ ...prev, [opKey]: false }));
    }
  };

  const viewReport = async (filename: string) => {
    try {
      const view = await invoke<ReportView>("read_report", { filename });
      setSelectedReport(view);
      setSelectedFilename(filename);
    } catch (e) {
      setError(`read_report failed: ${e}`);
    }
  };

  const openPdf = async (filename: string) => {
    try {
      await invoke("open_report_pdf", { filename });
    } catch (e) {
      setError(`open_report_pdf failed: ${e}`);
    }
  };

  const toggleSync = async (enabled: boolean) => {
    try {
      setSyncStatus(await invoke<SyncStatus>("set_sync_enabled", { enabled }));
      setError("");
    } catch (e) {
      setError(`sync toggle failed: ${e}`);
    }
  };

  const runSyncNow = async () => {
    setSyncBusy(true);
    setError("");
    try {
      setSyncStatus(await invoke<SyncStatus>("sync_now"));
    } catch (e) {
      setError(`sync failed: ${e}`);
    } finally {
      setSyncBusy(false);
    }
  };

  const activeOps = Object.values(active);
  const statusColor = (verified: boolean) => (verified ? "#34c759" : "#ff453a");

  return (
    <main className="dashboard">
      <header>
        <h1>Purgent</h1>
        <p>
          Standards-compliant secure erasure · forensic recovery · tamper-evident audit trail
        </p>
      </header>

      <section className="identity-bar">
        <div>
          <label>Operator ID</label>
          <span className="inline-row">
            <input value={operatorInput} onChange={(e) => setOperatorInput(e.target.value)} />
            <button onClick={updateOperator}>Set identity</button>
          </span>
        </div>
        {identity && (
          <div className="identity-meta">
            <div className="mono">fingerprint {identity.fingerprint}</div>
            <div className="mono">reports → {identity.report_dir}</div>
          </div>
        )}
      </section>

      {error && (
        <section className="banner error">
          <span>{error}</span>
          <button onClick={() => setError("")}>dismiss</button>
        </section>
      )}

      <nav className="tabs">
        {(
          [
            ["wipe", "Secure Erase"],
            ["erase", "Erase Files"],
            ["carve", "Recovery"],
            ["reports", "Reports & Compliance"],
          ] as [Tab, string][]
        ).map(([id, label]) => (
          <button
            key={id}
            className={tab === id ? "tab active" : "tab"}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </nav>

      <section className="panel grid-2">
        <div>
          {tab === "wipe" && (
            <div className="task">
              <h2>Secure Erase</h2>
              <label>Target type</label>
              <select
                value={wipeTargetKind}
                onChange={(e) => setWipeTargetKind(e.target.value as "device" | "file")}
              >
                <option value="file">Image file (recommended)</option>
                <option value="device">Physical device (admin required)</option>
              </select>

              {wipeTargetKind === "file" ? (
                <>
                  <label>Path to disk image</label>
                  <input
                    placeholder="C:\path\to\disk.img"
                    value={wipePath}
                    onChange={(e) => {
                      setWipePath(e.target.value);
                      setWipeConfirm("");
                    }}
                  />
                </>
              ) : (
                <>
                  <label>Device</label>
                  <select
                    value={wipeDevicePath}
                    onChange={(e) => {
                      setWipeDevicePath(e.target.value);
                      setWipeConfirm("");
                    }}
                  >
                    <option value="">select…</option>
                    {devices.map((d) => (
                      <option key={d.id} value={d.id}>
                        {d.id} · {d.model || d.media_type} · {formatBytes(d.capacity_bytes)}
                      </option>
                    ))}
                  </select>
                  {selectedDevice && (
                    <p className="hint">
                      This wipes every sector of <span className="mono">{selectedDevice.id}</span>.
                      Ensure the correct drive is selected; data cannot be recovered.
                    </p>
                  )}
                </>
              )}

              <label>Standard</label>
              <select value={wipeStandard} onChange={(e) => setWipeStandard(e.target.value)}>
                {wipeStandards.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.label}
                  </option>
                ))}
              </select>

              <label>Confirm target — type exactly</label>
              <div className="conf">
                <code>{wipeExpected || "…"}</code>
                <input
                  placeholder="type the exact phrase above"
                  value={wipeConfirm}
                  onChange={(e) => setWipeConfirm(e.target.value)}
                />
              </div>

              <button
                className="run"
                disabled={!wipeReady}
                onClick={runWipe}
              >
                Begin secure erase
              </button>
              <p className="hint">
                The exact phrase must match the selected target; a mismatch refuses the operation.
              </p>
            </div>
          )}

          {tab === "erase" && (
            <div className="task">
              <h2>Erase Files / Folder</h2>
              <label>Target type</label>
              <select
                value={eraseKind}
                onChange={(e) => {
                  setEraseKind(e.target.value as "file" | "folder");
                  setEraseConfirm("");
                }}
              >
                <option value="file">Single file (overwrite + delete)</option>
                <option value="folder">Folder (all contained files)</option>
              </select>
              <label>Path</label>
              <input
                placeholder={eraseKind === "file" ? "C:\path\to\file" : "C:\path\to\folder"}
                value={erasePath}
                onChange={(e) => {
                  setErasePath(e.target.value);
                  setEraseConfirm("");
                }}
              />
              <label>Standard</label>
              <select value={eraseStandard} onChange={(e) => setEraseStandard(e.target.value)}>
                {eraseStandards.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.label}
                  </option>
                ))}
              </select>
              <label>Confirm target — type exactly</label>
              <div className="conf">
                <code>{eraseExpected || "…"}</code>
                <input
                  placeholder="type the exact phrase above"
                  value={eraseConfirm}
                  onChange={(e) => setEraseConfirm(e.target.value)}
                />
              </div>
              <button className="run" disabled={!eraseReady} onClick={runErase}>
                Begin file erase
              </button>
            </div>
          )}

          {tab === "carve" && (
            <div className="task">
              <h2>Recovery (carve)</h2>
              <p className="hint">
                Scans a source read-only for embedded file signatures and reconstructs files into
                the output folder. Recovered files are validated and scored.
              </p>
              <label>Source (read-only)</label>
              <input
                placeholder="C:\path\to\source.img"
                value={carveSource}
                onChange={(e) => setCarveSource(e.target.value)}
              />
              <label>Output folder</label>
              <input
                placeholder="C:\path\to\recovered"
                value={carveOutput}
                onChange={(e) => setCarveOutput(e.target.value)}
              />
              <button className="run" disabled={!carveReady} onClick={runCarve}>
                Begin recovery
              </button>
            </div>
          )}

          {tab === "reports" && (
            <div className="reports-view">
              <h2>Audit Trail</h2>
              <div className="sync-panel">
                <h3>Cloud sync (Supabase)</h3>
                {syncStatus ? (
                  <>
                    <p className="hint">
                      {syncStatus.configured
                        ? `endpoint ${syncStatus.url}`
                        : "not configured — set PURGENT_SUPABASE_URL and PURGENT_SUPABASE_ANON_KEY."}
                    </p>
                    <p className="hint">
                      synced {syncStatus.synced} of {syncStatus.total} report
                      {syncStatus.total === 1 ? "" : "s"} · audit metadata only, recovered file
                      contents never leave this device
                    </p>
                    <div className="inline-row">
                      <button
                        disabled={!syncStatus.configured}
                        onClick={() => toggleSync(!syncStatus.enabled)}
                      >
                        {syncStatus.enabled ? "Disable sync" : "Enable sync"}
                      </button>
                      <button
                        disabled={!syncStatus.enabled || syncBusy}
                        onClick={runSyncNow}
                      >
                        {syncBusy ? "Syncing…" : "Sync now"}
                      </button>
                      {syncStatus.enabled && (
                        <span className="badge" style={{ color: "#34c759", borderColor: "#34c759" }}>
                          sync enabled
                        </span>
                      )}
                    </div>
                  </>
                ) : (
                  <p className="hint">sync status unavailable.</p>
                )}
              </div>
              <table>
                <thead>
                  <tr>
                    <th>Status</th>
                    <th>Type</th>
                    <th>Target</th>
                    <th>Standard</th>
                    <th>Operator</th>
                    <th>Started</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {reports.length === 0 && (
                    <tr>
                      <td colSpan={7} className="empty">
                        No reports yet — run an operation to create one.
                      </td>
                    </tr>
                  )}
                  {reports.map((r) => (
                    <tr key={r.filename}>
                      <td>
                        <span
                          className="badge"
                          style={{ color: statusColor(r.verified), borderColor: statusColor(r.verified) }}
                        >
                          {r.verified ? "verified" : "INVALID"}
                        </span>
                      </td>
                      <td className="mono">{r.operation_type}</td>
                      <td className="mono" title={r.target}>
                        {r.target.length > 42 ? "…" + r.target.slice(-42) : r.target}
                      </td>
                      <td>{r.standard_label}</td>
                      <td>{r.operator_id}</td>
                      <td>{shortTime(r.start_time)}</td>
                      <td>
                        <button onClick={() => viewReport(r.filename)}>view</button>{" "}
                        {r.has_pdf && <button onClick={() => openPdf(r.filename)}>pdf</button>}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              <h2>Compliance Matrix</h2>
              <table>
                <thead>
                  <tr>
                    <th>Report</th>
                    <th>Operation</th>
                    <th>Verification</th>
                    <th>Mismatch</th>
                    <th>Signature</th>
                    <th>Hash</th>
                  </tr>
                </thead>
                <tbody>
                  {reports.length === 0 && (
                    <tr>
                      <td colSpan={6} className="empty">
                        No rows yet.
                      </td>
                    </tr>
                  )}
                  {reports.map((r) => (
                    <tr
                      key={r.filename}
                      className={selectedFilename === r.filename ? "row-selected" : ""}
                      onClick={() => viewReport(r.filename)}
                    >
                      <td className="mono">{r.filename}</td>
                      <td className="mono">{r.operation_type}</td>
                      <td>{selectedFilename === r.filename && selectedReport?.report.verification ? (
                        <span style={{ color: statusColor(selectedReport.report.verification.status === "passed") }}>
                          {selectedReport.report.verification.status}
                        </span>
                      ) : "—"}
                      </td>
                      <td>{selectedFilename === r.filename ? selectedReport?.report.verification.mismatched_sectors ?? "—" : "—"}</td>
                      <td style={{ color: statusColor(r.verified) }}>{r.verified ? "valid HMAC" : "INVALID"}</td>
                      <td className="mono">
                        {selectedFilename === r.filename ? shortId(selectedReport?.report?.report_hash) : "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        <div className="monitor">
          <h2>Progress Monitor</h2>
          {activeOps.length === 0 && <p className="hint">No active operation.</p>}
          {activeOps.map((op) => {
            const pct = op.total_bytes > 0 ? Math.round((op.bytes_done / op.total_bytes) * 100) : 0;
            return (
              <div key={op.operation_id} className="op-card">
                <div className="op-head">
                  <span className="mono">{op.operation_type}</span>
                  <span>{op.phase}</span>
                </div>
                <div className="bar">
                  <div className="bar-fill" style={{ width: `${pct}%` }} />
                </div>
                <div className="op-foot mono">
                  {formatBytes(op.bytes_done)} / {formatBytes(op.total_bytes)} · {pct}%
                </div>
                <div className="hint">{op.message}</div>
                <div className="mono tiny">{op.operation_id}</div>
              </div>
            );
          })}
          {lastOutcome && (
            <div className="op-card done">
              <div className="op-head">
                <span>Last completed operation</span>
                <span className="mono">{lastOutcome.operation_id}</span>
              </div>
              <div className="mono tiny">
                {lastOutcome.report_json.replace(/\\/g, "/")}
              </div>
            </div>
          )}
          {Object.keys(busy).length > 0 && <p className="hint">operation running…</p>}
        </div>
      </section>

      {selectedReport && (
        <section className="panel report-json">
          <h2>
            Report Detail <span className="mono">{selectedFilename}</span>
            <span
              className="badge"
              style={{
                color: statusColor(selectedReport.verified),
                borderColor: statusColor(selectedReport.verified),
              }}
            >
              {selectedReport.verified ? "signature verified" : "signature INVALID"}
            </span>
          </h2>
          <pre>{JSON.stringify(selectedReport.report, null, 2)}</pre>
        </section>
      )}
    </main>
  );
}

function shortId(hash?: string): string {
  if (!hash) return "—";
  return hash.length > 12 ? hash.slice(0, 12) + "…" : hash;
}

export default App;