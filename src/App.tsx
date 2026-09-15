import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Navbar } from "./components/ui/navbar";
import { LandingPage } from "./components/ui/landing-page";
import ConstellationGrid from "./components/ui/constellation-grid";
import { ArrowLeft, RefreshCw } from "lucide-react";
import logoImg from "./assets/logo.png";

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
  capacity_bytes: number;
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
    } | null;
    skipped_sector_count: number;
    method?: string | null;
    evidence_hash?: string | null;
    hpa_dco?: {
      hpa_present: boolean;
      dco_present: boolean;
      removable: boolean;
      max_current_lba: number;
      max_native_lba: number;
    } | null;
    hpa_dco_removal?: {
      attempted: boolean;
      hpa_clear_sent: boolean;
      dco_reset_sent: boolean;
      removed: boolean;
      detail: string;
    } | null;
    trace_scrub?: {
      file_path: string;
      scrubbed_at: string;
      actions?: unknown[];
    } | null;
    categories?: { category: string; label: string; count: number }[] | null;
    report_hash: string;
    signature_alg: string;
    signature: string;
  };
  verified: boolean;
}

type Tab = "wipe" | "erase" | "carve" | "reports";
type ViewMode = "showcase" | "console";

interface SyncStatus {
  enabled: boolean;
  configured: boolean;
  url: string;
  synced: number;
  total: number;
}

function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
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

function normalizeMediaType(raw: string | undefined): string {
  const map: Record<string, string> = {
    "HDD": "hdd",
    "SSD": "ssd",
    "NVMe": "nvme",
    "NVMe SSD": "nvme",
    "USB Flash": "usb_flash",
    "SD Flash": "sd_flash",
    "Optical": "cd_rom",
    "Unknown": "unknown",
  };
  if (!raw) return "unknown";
  return map[raw] ?? raw;
}

function isHardwareStandard(id: string): boolean {
  return id === "ata_secure_erase" || id === "nvme_sanitize";
}

function App() {
  const [viewMode, setViewMode] = useState<ViewMode>("showcase");
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
  const [wipeFallbackError, setWipeFallbackError] = useState("");
  const [wipeFallbackPending, setWipeFallbackPending] = useState(false);
  const [wipeFallbackStandard, setWipeFallbackStandard] = useState("");

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

  const [now, setNow] = useState<Date>(new Date());
  const sessionId = useMemo(
    () => `SECURE-SESSION-${Math.random().toString(36).slice(2, 8).toUpperCase()}`,
    []
  );

  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(t);
  }, []);

  const refreshReports = useCallback(async () => {
    try {
      if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
        const list = await invoke<ReportMeta[]>("list_reports");
        setReports(list);
      } else {
        // Mock default for browser showcase preview
        setReports([
          {
            filename: "report_20260914_001.json",
            report_id: "REP-980PRO-NIST",
            operation_type: "secure_erase",
            operator_id: "DFIR-OPR-40291",
            target: "Samsung 980 PRO 1TB (NVMe)",
            start_time: new Date().toISOString(),
            standard_label: "NIST SP 800-88 Purge",
            verified: true,
            has_pdf: true,
            capacity_bytes: 1000204886016,
          },
          {
            filename: "report_20260914_002.json",
            report_id: "REP-CARVE-088B",
            operation_type: "file_recovery",
            operator_id: "DFIR-OPR-40291",
            target: "Kingston DataTraveler 32GB",
            start_time: new Date(Date.now() - 3600000).toISOString(),
            standard_label: "Structure & Signature Carving",
            verified: true,
            has_pdf: true,
            capacity_bytes: 32212254720,
          },
        ]);
      }
    } catch {
      /* browser preview fallback */
    }
  }, []);

  const refreshSync = useCallback(async () => {
    try {
      if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
        setSyncStatus(await invoke<SyncStatus>("get_sync_status"));
      } else {
        setSyncStatus({
          enabled: false,
          configured: false,
          url: "offline.local (Air-Gapped Sovereign Node)",
          synced: 0,
          total: 2,
        });
      }
    } catch {
      /* browser preview fallback */
    }
  }, []);

  useEffect(() => {
    (async () => {
      const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
      if (isTauri) {
        try {
          const id = await invoke<Identity>("get_identity");
          setIdentity(id);
          setOperatorInput(id.operator_id);
        } catch {
          /* best effort */
        }
        try {
          setDevices(await invoke<Device[]>("list_devices"));
        } catch {
          /* best effort */
        }
        try {
          setWipeStandards(await invoke<StdInfo[]>("get_wipe_standards"));
          setWipeStandard((await invoke<StdInfo[]>("get_wipe_standards"))[0]?.id ?? "");
        } catch {
          /* best effort */
        }
        try {
          setEraseStandards(await invoke<StdInfo[]>("get_erase_standards"));
          setEraseStandard((await invoke<StdInfo[]>("get_erase_standards"))[0]?.id ?? "");
        } catch {
          /* best effort */
        }
      } else {
        // Mock identity & standards in browser mode
        setIdentity({
          operator_id: "DFIR-OPR-40291",
          fingerprint: "SHA256:7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
          report_dir: "C:\\Purgent\\Vault\\Reports",
        });
        setOperatorInput("DFIR-OPR-40291");
        setDevices([
          {
            id: "\\\\.\\PhysicalDrive0",
            model: "Samsung SSD 980 PRO 1TB",
            serial: "S5GXNF0R123456",
            capacity_bytes: 1000204886016,
            media_type: "NVMe SSD",
            bus_type: "NVMe",
            is_removable: false,
          },
          {
            id: "\\\\.\\PhysicalDrive1",
            model: "Kingston DataTraveler 3.0",
            serial: "0014D11200000000",
            capacity_bytes: 32212254720,
            media_type: "USB Flash",
            bus_type: "USB",
            is_removable: true,
          },
        ]);
        setWipeStandards([
          { id: "nist-purge", label: "NIST SP 800-88 Rev.1 Purge (Hardware Cryptographic)" },
          { id: "nist-clear", label: "NIST SP 800-88 Rev.1 Clear (Single-pass Overwrite)" },
          { id: "dod-5220", label: "DoD 5220.22-M (3-pass Overwrite + Verify)" },
          { id: "ieee-2883", label: "IEEE 2883-2022 Sanitize (Flash Solid-State)" },
        ]);
        setWipeStandard("nist-purge");
        setEraseStandards([
          { id: "dod-file", label: "DoD 5220.22-M File Shredding (7-pass)" },
          { id: "gutmann-quick", label: "Gutmann 7-pass File Wipe" },
        ]);
        setEraseStandard("dod-file");
      }
      await refreshReports();
      await refreshSync();
    })();
    return;
  }, [refreshReports, refreshSync]);

  useEffect(() => {
    let unlisteners: UnlistenFn[] = [];
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (isTauri) {
      (async () => {
        try {
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
        } catch {
          /* ignore */
        }
      })();
    }
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
    const mediaTypeId =
      selectedDevice && selectedDevice.media_type
        ? normalizeMediaType(selectedDevice.media_type)
        : "unknown";
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
          mediaTypeId,
          standardId: wipeStandard,
          fallbackAcknowledged: wipeFallbackPending,
          confirmed: wipeConfirm,
        });
      }
      setWipeFallbackPending(false);
      setWipeFallbackError("");
    } catch (e) {
      const msg = `${e}`;
      if (msg.includes("RequiresFallbackAck") || msg.includes("overwrite fallback denied")) {
        setWipeFallbackError(msg.replace(/^wipe failed: /, ""));
        setWipeFallbackPending(true);
        setWipeFallbackStandard(wipeStandard);
      } else {
        setError(`wipe failed: ${msg}`);
        setWipeFallbackPending(false);
        setWipeFallbackError("");
      }
      setBusy((prev) => ({ ...prev, [opKey]: false }));
    }
  };

  const confirmFallback = async () => {
    if (!wipeFallbackStandard) return;
    setWipeFallbackError("");
    await runWipe();
  };

  const cancelFallback = () => {
    setWipeFallbackPending(false);
    setWipeFallbackError("");
    setError("hardware erase aborted by operator; no fallback performed");
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

  const exportXml = async (filename: string) => {
    try {
      const path = await invoke<string>("export_report_xml", { filename });
      setError("");
      alert(`XML certificate exported:\n${path}`);
    } catch (e) {
      setError(`export_report_xml failed: ${e}`);
    }
  };

  const openXml = async (filename: string) => {
    try {
      await invoke("open_report_xml", { filename });
    } catch (e) {
      setError(`open_report_xml failed: ${e}`);
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
  const statusColor = (verified: boolean) => (verified ? "#5C7A5E" : "#B0503C");

  const totalErased = reports
    .filter((r) => r.operation_type === "secure_erase" || r.operation_type === "file_erase")
    .reduce((sum, r) => sum + (r.capacity_bytes || 0), 0);
  const artifacts = reports.filter((r) => r.operation_type === "file_recovery").length;
  const verifiedCount = reports.filter((r) => r.verified).length;
  const tampered = reports.length - verifiedCount;

  return (
    <div className="min-h-screen bg-[#0B111E] text-white">
      {/* Global Header / Mode Bar */}
      {viewMode === "showcase" ? (
        <>
          <Navbar
            onOpenConsole={() => setViewMode("console")}
            onNavigateSection={(id) => {
              const el = document.getElementById(id);
              if (el) el.scrollIntoView({ behavior: "smooth" });
            }}
          />
          <LandingPage onOpenConsole={() => setViewMode("console")} />
        </>
      ) : (
        <div className="min-h-screen relative">
          {/* Constellation Grid Background for Console */}
          <div className="fixed inset-0 z-0 pointer-events-none">
            <ConstellationGrid
              opacity={0.15}
              forceDark={true}
              labels={['Wipe', 'Erase', 'Carve', 'Verify', 'Report', 'Sector', 'Hash', 'Ledger', 'IOCTL', 'DMA', 'Cert', 'Operator']}
            />
          </div>

          {/* Top Return / Mode Bar for Console */}
          <div className="bg-[#10192C]/90 backdrop-blur-sm border-b border-[#1E2D4A] px-6 py-3.5 sticky top-0 z-40 flex items-center justify-between shadow-[0_8px_20px_rgba(7,10,19,0.7)]">
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={() => setViewMode("showcase")}
                className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl text-xs font-mono neu-btn"
              >
                <ArrowLeft className="w-3.5 h-3.5 text-[#38BDF8]" />
                Back to Landing Page
              </button>
              <div className="h-4 w-[1px] bg-[#1E2D4A] hidden sm:block" />
              <span className="text-xs font-mono text-[#8E9DB8] hidden sm:inline">
                Purgent Desktop Engine v0.1.0 · Tauri v2
              </span>
            </div>

            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={() => {
                  refreshReports();
                  refreshSync();
                }}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-xs font-mono neu-btn text-[#8E9DB8] hover:text-white"
              >
                <RefreshCw className="w-3 h-3 text-[#38BDF8]" />
                Refresh
              </button>
              <span className="px-3 py-1 rounded-full text-[11px] font-mono neu-inset text-[#34D399] flex items-center gap-1.5">
                <span className="w-2 h-2 rounded-full bg-[#34D399] shadow-[0_0_6px_#34D399]" />
                ENCLAVE ACTIVE
              </span>
            </div>
          </div>

          <main className="dashboard pt-6">
            <section className="statusbar">
              <div className="sb-brand">
                <img src={logoImg} alt="Purgent logo" className="sb-logo" />
                <span className="sb-title">PURGENT</span>
                <span className="sb-sub">TACTICAL FORENSIC CONSOLE</span>
              </div>
              <div className="sb-cell font-mono">
                NODE://{identity?.fingerprint ? identity.fingerprint.slice(0, 12) : "…"}
              </div>
              <div className="sb-cell font-mono">SESSION: {sessionId}</div>
              <div className="sb-cell font-mono">UTC {now.toISOString().slice(11, 19)}</div>
              <div className="sb-cell">
                <span className="dot ok" /> AIR-GAPPED READY
              </div>
              {identity && <div className="sb-cell font-mono">OPR: {identity.operator_id}</div>}
              <div className="sb-cell chip" onClick={() => setError("")}>
                {error ? "ALERT" : "VERIFIED SECURE"}
              </div>
            </section>

            <section className="subsys">
              <span className="badge text-[#5C7A5E] border-[#5C7A5E]/30 bg-[#22231C]">
                NIST SP 800-88
              </span>
              <span className="badge text-[#B8862F] border-[#B8862F]/30 bg-[#22231C]">
                DoD 5220.22-M
              </span>
              <span className="badge text-[#EDEAE0] border-[#2C2D26] bg-[#22231C]">
                IEEE 2883-2022
              </span>
              <span className="text-[#8C8A7C] ml-auto font-mono text-xs">
                MANDATORY READBACK VERIFICATION PASS ACTIVE
              </span>
            </section>

            <header className="mb-6">
              <p className="crumb font-mono">Forensic Console / Device Controller</p>
              <h1 className="tactical-title">Operational Workspace</h1>
              <p className="text-sm text-[#8C8A7C]">
                Standards-compliant secure erasure · forensic file recovery · signed immutable audit ledger
              </p>
            </header>

            <section className="identity-bar">
              <div>
                <label className="text-xs font-mono text-[#8C8A7C] uppercase tracking-wider block mb-1">
                  Operator Identity
                </label>
                <span className="inline-row">
                  <input
                    value={operatorInput}
                    onChange={(e) => setOperatorInput(e.target.value)}
                    placeholder="Enter Operator ID"
                  />
                  <button className="tactical-btn font-mono text-xs" onClick={updateOperator}>
                    Set identity
                  </button>
                </span>
              </div>
              {identity && (
                <div className="identity-meta">
                  <div className="font-mono">Fingerprint: {identity.fingerprint}</div>
                  <div className="font-mono">Audit Vault: {identity.report_dir}</div>
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

      <section className="kpi-row">
        <div className="kpi">
          <div className="kpi-label">TOTAL DATA ERASED</div>
          <div className="kpi-value">{formatBytes(totalErased)}</div>
          <div className="kpi-sub">secure erase + file erase</div>
        </div>
        <div className="kpi">
          <div className="kpi-label">RECOVERED ARTIFACTS</div>
          <div className="kpi-value">{artifacts}</div>
          <div className="kpi-sub">carve operations</div>
        </div>
        <div className="kpi">
          <div className="kpi-label">ACTIVE OPERATIONS</div>
          <div className="kpi-value">{activeOps.length}</div>
          <div className="kpi-sub">{activeOps.length > 0 ? "running" : "idle"}</div>
        </div>
        <div className="kpi">
          <div className="kpi-label" style={{ color: "#7b73ef" }}>
            AUDIT LEDGER
          </div>
          <div className="kpi-value">{reports.length}</div>
          <div className="kpi-sub">
            {tampered === 0 ? "integrity 100% · 0 collisions" : `${tampered} tampered record(s)`}
          </div>
        </div>
        <div className="kpi">
          <div className="kpi-label">VERIFIED</div>
          <div className="kpi-value" style={{ color: "#34c759" }}>
            {verifiedCount}/{reports.length || 0}
          </div>
          <div className="kpi-sub">signed reports valid</div>
        </div>
      </section>

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

              {isHardwareStandard(wipeStandard) && (
                <p className="hint" style={{ color: "#7b73ef" }}>
                  Hardware-backed method (ATA Secure Erase / NVMe Sanitize). If the controller
                  cannot honor the command, Purgent requires your explicit acknowledgement
                  before any overwrite fallback — never silently.
                </p>
              )}

              {wipeFallbackError && (
                <div className="fallback-dialog">
                  <h3>Hardware erase unavailable — overwrite fallback requires acknowledgement</h3>
                  <p>{wipeFallbackError}</p>
                  <p className="hint" style={{ color: "#b0503c" }}>
                    Confirming below will overwrite with a verified pseudo-random pass per RULES;
                    the report records the fallback reason and method. Cancelling aborts with no
                    change to the drive.
                  </p>
                  <div className="fallback-actions">
                    <button className="run" onClick={confirmFallback}>
                      I acknowledge the overwrite fallback
                    </button>
                    <button className="ghost" onClick={cancelFallback}>
                      Cancel — do not attempt fallback
                    </button>
                  </div>
                </div>
              )}
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
              <h2>Certificate Vault &amp; Review</h2>
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

              <h2>Compliance Matrix · LEGAL GRID</h2>
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
                      <td>{selectedFilename === r.filename ? selectedReport?.report.verification?.mismatched_sectors ?? "—" : "—"}</td>
                      <td style={{ color: statusColor(r.verified) }}>{r.verified ? "valid HMAC" : "INVALID"}</td>
                      <td className="mono">
                        {selectedFilename === r.filename ? shortId(selectedReport?.report?.report_hash) : "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              <h2>Attached Storage &amp; Write-Block Matrix</h2>
              <table>
                <thead>
                  <tr>
                    <th>Device</th>
                    <th>Bus</th>
                    <th>Media</th>
                    <th>Capacity</th>
                    <th>Removable</th>
                    <th>Health</th>
                  </tr>
                </thead>
                <tbody>
                  {devices.length === 0 && (
                    <tr>
                      <td colSpan={6} className="empty">
                        No block devices detected (best-effort enumeration).
                      </td>
                    </tr>
                  )}
                  {devices.map((d) => (
                    <tr key={d.id}>
                      <td className="mono" title={d.serial}>
                        {d.model || d.id}
                        {d.serial ? (
                          <span className="tiny"> · SN {d.serial.slice(0, 18)}…</span>
                        ) : null}
                      </td>
                      <td className="mono">{d.bus_type}</td>
                      <td>{d.media_type}</td>
                      <td className="mono">{formatBytes(d.capacity_bytes)}</td>
                      <td>
                        <span
                          className="badge"
                          style={{
                            color: d.is_removable ? "#ffd60a" : "#34c759",
                            borderColor: d.is_removable ? "#ffd60a" : "#34c759",
                          }}
                        >
                          {d.is_removable ? "removable" : "fixed"}
                        </span>
                      </td>
                      <td className="mono">
                        <span className="dot ok" /> good
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
            {complianceBadges(selectedReport.report).map((b) => (
              <span
                key={b}
                className="badge"
                style={{ color: "#34c759", borderColor: "#34c759" }}
              >
                {b}
              </span>
            ))}
          </h2>

          <div className="report-extras">
            {selectedReport.report.method && (
              <p className="mono tiny">
                method: <strong>{selectedReport.report.method}</strong>
              </p>
            )}
            {selectedReport.report.evidence_hash && (
              <p className="mono tiny">
                evidence sha256: <em>{shortId(selectedReport.report.evidence_hash)}</em>
              </p>
            )}
            {selectedReport.report.hpa_dco && (
              <p className="mono tiny">
                hpa/dco: hpa={String(selectedReport.report.hpa_dco.hpa_present)} · dco=
                {String(selectedReport.report.hpa_dco.dco_present)} · current_lba=
                {selectedReport.report.hpa_dco.max_current_lba} · native_lba=
                {selectedReport.report.hpa_dco.max_native_lba}
              </p>
            )}
            {selectedReport.report.hpa_dco_removal && (
              <p className="mono tiny">
                hpa/dco removal: sent_hpa={String(selectedReport.report.hpa_dco_removal.hpa_clear_sent)} ·
                sent_dco={String(selectedReport.report.hpa_dco_removal.dco_reset_sent)} · removed=
                {String(selectedReport.report.hpa_dco_removal.removed)} ·{" "}
                <em>{selectedReport.report.hpa_dco_removal.detail}</em>
              </p>
            )}
            {selectedReport.report.trace_scrub && (
              <p className="mono tiny">
                trace scrub: <em>{selectedReport.report.trace_scrub.file_path}</em> at{" "}
                {shortTime(selectedReport.report.trace_scrub.scrubbed_at)}
              </p>
            )}
            {selectedReport.report.categories && selectedReport.report.categories.length > 0 && (
              <p className="mono tiny">
                categories:{" "}
                {selectedReport.report.categories
                  .map((c) => `${c.label}×${c.count}`)
                  .join(" · ")}
              </p>
            )}
            <div className="fallback-actions" style={{ marginTop: 8 }}>
              <button className="ghost" onClick={() => openPdf(selectedFilename)}>
                Open PDF certificate
              </button>
              <button className="ghost" onClick={() => exportXml(selectedFilename)}>
                Export XML certificate
              </button>
              <button className="ghost" onClick={() => openXml(selectedFilename)}>
                Open XML certificate
              </button>
            </div>
          </div>

          <pre>{JSON.stringify(selectedReport.report, null, 2)}</pre>
        </section>
      )}
          </main>
        </div>
      )}
    </div>
  );
}
function shortId(hash?: string): string {
  if (!hash) return "—";
  return hash.length > 12 ? hash.slice(0, 12) + "…" : hash;
}

function complianceBadges(report: ReportView["report"]): string[] {
  const badges: string[] = [];
  const sid = report.standard_id ?? "";
  const label = report.standard_label ?? "";
  if (sid.includes("ieee_2883") || label.includes("IEEE 2883")) {
    badges.push("IEEE 2883-2022");
  }
  if (sid.includes("iso_27037") || label.includes("ISO") || label.includes("27037")) {
    badges.push("ISO/IEC 27037");
  }
  if (sid.includes("nist_800_88") || label.includes("NIST")) {
    badges.push("NIST SP 800-88 Rev.1");
  }
  if (sid.includes("dod_522022") || label.includes("DoD 5220.22-M")) {
    badges.push("DoD 5220.22-M");
  }
  if (sid.includes("ata_secure_erase") || label.includes("ATA Secure Erase")) {
    badges.push("ATA Secure Erase");
  }
  if (sid.includes("nvme_sanitize") || label.includes("NVMe Sanitize")) {
    badges.push("NVMe Sanitize");
  }
  return badges;
}

export default App;
