import { ShieldCheck } from "lucide-react";
import ConstellationGrid from "./constellation-grid";

interface HeroProps {
  onOpenConsole?: () => void;
}

export function Hero({ onOpenConsole }: HeroProps) {
  return (
    <section className="relative pt-36 pb-24 md:pt-44 md:pb-32 overflow-hidden">
      {/* Constellation Grid Background */}
      <div className="absolute inset-0 z-0">
        <ConstellationGrid opacity={0.4} forceDark={true} />
      </div>

      {/* Background Soft Blue Glow Elements */}
      <div className="absolute top-1/4 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] bg-gradient-to-tr from-blue-600/10 to-sky-400/10 rounded-full blur-3xl pointer-events-none z-[1]" />

      <div className="max-w-[1520px] mx-auto px-6 sm:px-8 xl:px-12 relative z-10">
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-10 xl:gap-14 items-center">
          {/* Left Column: Typography & Intent */}
          <div className="lg:col-span-7 flex flex-col items-start max-w-3xl">
            {/* Eyebrow Label with Neomorphic Inset Pill */}
            <div className="inline-flex items-center gap-2.5 px-4 py-1.5 rounded-full neu-inset text-xs text-[#8E9DB8] font-mono mb-6">
              <span className="w-2 h-2 rounded-full bg-[#38BDF8] shadow-[0_0_8px_#38BDF8]" />
              <span className="text-white font-medium">NIST SP 800-88 & DoD 5220.22-M Compliant</span>
            </div>

            {/* Headline with serif display font Fraunces in Crisp White */}
            <h1 className="font-serif text-4xl sm:text-5xl lg:text-6xl font-semibold tracking-tight text-white leading-[1.12] mb-6 drop-shadow-[0_4px_20px_rgba(0,0,0,0.5)]">
              Sanitize with certainty. <br />
              <span className="text-transparent bg-clip-text bg-gradient-to-r from-sky-400 via-blue-400 to-indigo-300">
                Recover with proof.
              </span>
            </h1>

            {/* Supporting paragraph in clean high-contrast text */}
            <p className="text-base sm:text-lg text-[#8E9DB8] font-sans font-normal leading-relaxed max-w-xl mb-9">
              Purgent integrates certified physical data sanitization and forensic-grade carving into a single tamper-evident ledger. Pure cryptographic certainty.
            </p>

            {/* Primary CTA + Secondary Link (Neomorphic Buttons) */}
            <div className="flex flex-wrap items-center gap-4 w-full sm:w-auto">
              <button
                type="button"
                onClick={onOpenConsole}
                className="w-full sm:w-auto px-7 py-3.5 rounded-xl neu-btn-primary text-sm font-semibold tracking-wide"
              >
                Launch Tactical Console
              </button>

              <a
                href="#standards"
                className="w-full sm:w-auto inline-flex items-center justify-center gap-2 px-6 py-3.5 rounded-xl neu-btn font-medium text-sm text-white"
              >
                <span>Read Verification Matrix</span>
              </a>
            </div>

            {/* Quiet confidence cues in Neomorphic Inset Cards */}
            <div className="grid grid-cols-3 gap-4 pt-10 mt-10 border-t border-[#1E2D4A] w-full max-w-2xl">
              <div className="p-4 rounded-xl neu-inset">
                <p className="font-mono text-[10px] text-[#8E9DB8] uppercase tracking-wider">Air-Gapped</p>
                <p className="text-sm font-semibold text-white mt-1">Zero Network Reqd</p>
              </div>
              <div className="p-4 rounded-xl neu-inset">
                <p className="font-mono text-[10px] text-[#8E9DB8] uppercase tracking-wider">Signatures</p>
                <p className="text-sm font-semibold text-white mt-1">HMAC & Ed25519</p>
              </div>
              <div className="p-4 rounded-xl neu-inset">
                <p className="font-mono text-[10px] text-[#8E9DB8] uppercase tracking-wider">Verification</p>
                <p className="text-sm font-semibold text-white mt-1">Mandatory Readback</p>
              </div>
            </div>
          </div>

          {/* Right Column: Neomorphic Forensic Certificate */}
          <div className="lg:col-span-5 relative w-full">
            <div className="relative rounded-3xl neu-card p-8 md:p-9 transition-transform duration-300 hover:scale-[1.01] w-full">
              {/* Header of Simulated Certificate */}
              <div className="flex items-center justify-between pb-5 mb-5 border-b border-[#1E2D4A]">
                <div className="flex items-center gap-3">
                  <div className="w-9 h-9 rounded-xl neu-pill flex items-center justify-center text-[#38BDF8]">
                    <ShieldCheck className="w-5 h-5 drop-shadow-[0_0_8px_rgba(56,189,248,0.6)]" />
                  </div>
                  <div>
                    <div className="text-xs font-mono font-bold text-white">CERT-2026-09-14-88A</div>
                    <div className="text-[11px] text-[#8E9DB8]">Cryptographic Sanitization Record</div>
                  </div>
                </div>
                <span className="px-3 py-1 rounded-full text-[10px] font-mono font-bold bg-[#16223B] text-[#34D399] shadow-[inset_1px_1px_3px_rgba(0,0,0,0.5),0_0_10px_rgba(52,211,153,0.3)]">
                  VERIFIED PASS
                </span>
              </div>

              {/* Certificate Details */}
              <div className="space-y-3 font-mono text-xs">
                <div className="flex justify-between py-2 border-b border-[#16223B]">
                  <span className="text-[#8E9DB8]">Standard</span>
                  <span className="text-white font-sans font-medium">NIST SP 800-88 Purge (ATA SE)</span>
                </div>
                <div className="flex justify-between py-2 border-b border-[#16223B]">
                  <span className="text-[#8E9DB8]">Target Device</span>
                  <span className="text-white">Samsung 980 PRO 1TB NVMe</span>
                </div>
                <div className="flex justify-between py-2 border-b border-[#16223B]">
                  <span className="text-[#8E9DB8]">Read-Back Sectors</span>
                  <span className="text-white">1,953,525,168 (100.0%)</span>
                </div>
                <div className="flex justify-between py-2 border-b border-[#16223B]">
                  <span className="text-[#8E9DB8]">Mismatched Bits</span>
                  <span className="text-[#34D399] font-semibold">0 (Zero residual signal)</span>
                </div>
                <div className="flex justify-between py-2 border-b border-[#16223B]">
                  <span className="text-[#8E9DB8]">Operator ID</span>
                  <span className="text-white font-semibold">DFIR-OPR-40291</span>
                </div>
                <div className="flex justify-between py-2 border-b border-[#16223B]">
                  <span className="text-[#8E9DB8]">Ledger Digest</span>
                  <span className="text-[#38BDF8] truncate max-w-[180px]">e3b0c44298fc1c149afbf4c8996fb924...</span>
                </div>
              </div>

              {/* Micro-activity bar in Neomorphic Inset Container */}
              <div className="mt-6 p-4 rounded-2xl neu-inset">
                <div className="flex items-center justify-between text-xs text-[#8E9DB8] mb-2.5 font-mono">
                  <span className="flex items-center gap-2 text-white font-medium">
                    <span className="w-2.5 h-2.5 rounded-full bg-[#34D399] animate-pulse shadow-[0_0_8px_#34D399]" />
                    Write-Blocked Carve Stream
                  </span>
                  <span>14,821 files</span>
                </div>
                <div className="w-full bg-[#070a13] h-2 rounded-full overflow-hidden shadow-[inset_1px_1px_3px_#000]">
                  <div className="bg-gradient-to-r from-sky-400 to-blue-500 h-full w-[84%] rounded-full shadow-[0_0_10px_rgba(56,189,248,0.7)]" />
                </div>
                <div className="flex justify-between items-center text-[11px] text-[#8E9DB8] font-mono mt-2">
                  <span>JPEG & PDF validated</span>
                  <span className="text-[#34D399] font-medium">Confidence: 99.4%</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
