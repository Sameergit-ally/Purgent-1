import { Shield } from "lucide-react";
import ConstellationGrid from "./constellation-grid";

interface CtaFooterProps {
  onOpenConsole?: () => void;
}

export function CtaFooter({ onOpenConsole }: CtaFooterProps) {
  return (
    <div>
      {/* Closing CTA Band with Neomorphic Card */}
      <section className="py-20 border-t border-[#1E2D4A] relative overflow-hidden">
        {/* Constellation Grid Background */}
        <div className="absolute inset-0 z-0 pointer-events-none">
          <ConstellationGrid
            opacity={0.25}
            forceDark={true}
            labels={['SHA-256', 'Ed25519', 'HMAC', 'Air-Gap', 'Ledger', 'PDF Cert', 'Audit', 'Operator', 'Immutable']}
          />
        </div>
        <div className="max-w-[1520px] mx-auto px-6 sm:px-8 xl:px-12 relative z-10">
          <div className="rounded-3xl neu-card-featured p-10 sm:p-14 md:p-20 relative overflow-hidden">
            <div className="relative z-10 max-w-4xl">
              <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full neu-inset text-xs font-mono text-[#38BDF8] mb-6">
                Forensic Assurance Ready
              </div>
              <h2 className="font-serif text-3xl sm:text-4xl lg:text-5xl font-semibold tracking-tight text-white mb-6">
                Replace fragmented sanitization with certified certainty.
              </h2>
              <p className="text-base sm:text-lg text-[#8E9DB8] font-sans leading-relaxed mb-9 max-w-3xl">
                Download the standalone, air-gapped desktop engine or launch the tactical console to execute standards-compliant wipes with readback verification.
              </p>
              <div className="flex flex-wrap items-center gap-4">
                <button
                  type="button"
                  onClick={onOpenConsole}
                  className="px-7 py-3.5 rounded-xl neu-btn-primary text-sm font-semibold"
                >
                  Launch Desktop Console
                </button>
                <a
                  href="#features"
                  className="px-6 py-3.5 rounded-xl neu-btn font-medium text-sm text-white"
                >
                  Review Architecture
                </a>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="bg-[#0B111E] border-t border-[#1E2D4A] pt-16 pb-12 text-sm">
        <div className="max-w-[1520px] mx-auto px-6 sm:px-8 xl:px-12">
          <div className="grid grid-cols-1 md:grid-cols-12 gap-10 pb-12 border-b border-[#16223B]">
            {/* Brand column (5 cols) */}
            <div className="md:col-span-5 space-y-4">
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-xl neu-pill flex items-center justify-center text-[#38BDF8]">
                  <Shield className="w-5 h-5 drop-shadow-[0_0_6px_rgba(56,189,248,0.5)]" />
                </div>
                <span className="font-serif text-xl font-bold text-white">
                  Purgent
                </span>
              </div>
              <p className="text-xs text-[#8E9DB8] max-w-sm leading-relaxed">
                Unified cross-platform forensic sanitization and recovery desktop application. Standards-first, verification-enforced, and backed by cryptographic chain-of-custody ledgers.
              </p>
              <div className="pt-2 text-xs font-mono text-[#8E9DB8]">
                Engine: Rust Core · Desktop: Tauri v2 · Storage: SQLite
              </div>
            </div>

            {/* Navigation Column 1 (2 cols) */}
            <div className="md:col-span-2 space-y-3">
              <div className="text-xs font-mono text-white uppercase tracking-wider font-semibold">
                Capabilities
              </div>
              <ul className="space-y-2 text-xs text-[#8E9DB8]">
                <li><a href="#features" className="hover:text-white transition-colors">NIST SP 800-88</a></li>
                <li><a href="#features" className="hover:text-white transition-colors">DoD 5220.22-M</a></li>
                <li><a href="#features" className="hover:text-white transition-colors">NVMe Sanitize</a></li>
                <li><a href="#features" className="hover:text-white transition-colors">File Carving</a></li>
                <li><a href="#features" className="hover:text-white transition-colors">Readback Verification</a></li>
              </ul>
            </div>

            {/* Navigation Column 2 (2 cols) */}
            <div className="md:col-span-2 space-y-3">
              <div className="text-xs font-mono text-white uppercase tracking-wider font-semibold">
                Compliance
              </div>
              <ul className="space-y-2 text-xs text-[#8E9DB8]">
                <li><a href="#features" className="hover:text-white transition-colors">Signed PDF Certificates</a></li>
                <li><a href="#features" className="hover:text-white transition-colors">Audit Ledger Schema</a></li>
                <li><a href="#features" className="hover:text-white transition-colors">Air-Gapped Operation</a></li>
                <li><a href="#features" className="hover:text-white transition-colors">Chain-of-Custody</a></li>
              </ul>
            </div>

            {/* Navigation Column 3: Status / Operational (3 cols) */}
            <div className="md:col-span-3 space-y-3">
              <div className="text-xs font-mono text-white uppercase tracking-wider font-semibold">
                System Status
              </div>
              <div className="p-4 rounded-2xl neu-inset space-y-2.5 text-xs font-mono">
                <div className="flex items-center justify-between">
                  <span className="text-[#8E9DB8]">Core Engine</span>
                  <span className="text-[#34D399] flex items-center gap-1.5 font-medium">
                    <span className="w-1.5 h-1.5 rounded-full bg-[#34D399] shadow-[0_0_6px_#34D399]" /> Operational
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-[#8E9DB8]">Ledger Verification</span>
                  <span className="text-[#38BDF8] font-medium">Active</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-[#8E9DB8]">Telemetry State</span>
                  <span className="text-white font-medium">Zero Outbound</span>
                </div>
              </div>
            </div>
          </div>

          {/* Bottom copyright line */}
          <div className="pt-8 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-[#8E9DB8]">
            <div>
              © 2026 Purgent Systems. Standards-compliant forensic sanitization.
            </div>
            <div className="flex items-center gap-6">
              <button
                type="button"
                onClick={onOpenConsole}
                className="hover:text-white transition-colors font-mono"
              >
                Launch Console
              </button>
              <a href="#features" className="hover:text-white transition-colors">
                Architecture
              </a>
            </div>
          </div>
        </div>
      </footer>
    </div>
  );
}
