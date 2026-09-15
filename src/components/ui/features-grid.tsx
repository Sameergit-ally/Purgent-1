import { 
  FileLock2, 
  SearchCode, 
  FileSignature, 
  HardDrive, 
  History,
  CheckCircle2
} from "lucide-react";
import ConstellationGrid from "./constellation-grid";

export function FeaturesGrid() {
  return (
    <section id="features" className="py-24 md:py-32 border-t border-[#1E2D4A] relative overflow-hidden">
      {/* Constellation Grid Background */}
      <div className="absolute inset-0 z-0 pointer-events-none">
        <ConstellationGrid
          opacity={0.2}
          forceDark={true}
          labels={['NVMe', 'ATA SE', 'IOCTL', 'Readback', 'Entropy', 'Sector', 'Block', 'Cluster', 'Fragment', 'Header', 'Trailer', 'xref']}
        />
      </div>
      <div className="max-w-[1520px] mx-auto px-6 sm:px-8 xl:px-12 relative z-10">
        {/* Section Header */}
        <div className="max-w-3xl mb-16">
          <div className="text-xs font-mono text-[#38BDF8] tracking-widest uppercase mb-3 font-semibold">
            Architectural Capabilities
          </div>
          <h2 className="font-serif text-3xl sm:text-4xl lg:text-5xl font-semibold tracking-tight text-white mb-4">
            Built for forensic labs, defense enclaves, and courtrooms.
          </h2>
          <p className="text-base text-[#8E9DB8] leading-relaxed">
            Conventional file shredders ignore wear-leveling in modern NVMe drives, while recovery tools lack verification chains. Purgent unifies both under one audited roof.
          </p>
        </div>

        {/* Asymmetric Neomorphic Bento Grid */}
        <div className="grid grid-cols-1 md:grid-cols-12 gap-7">
          {/* Dominant Featured Card (8 cols) */}
          <div className="md:col-span-8 rounded-2xl neu-card-featured p-9 flex flex-col justify-between">
            <div>
              <div className="w-12 h-12 rounded-xl neu-pill text-[#38BDF8] flex items-center justify-center mb-6">
                <FileSignature className="w-6 h-6 drop-shadow-[0_0_8px_rgba(56,189,248,0.6)]" />
              </div>
              <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-mono bg-[#16223B] text-[#34D399] neu-pill mb-4">
                Non-Negotiable Verification
              </div>
              <h3 className="text-xl sm:text-2xl font-sans font-semibold text-white mb-3">
                Mandatory read-back verification on every pass
              </h3>
              <p className="text-[#8E9DB8] text-sm sm:text-base leading-relaxed max-w-2xl mb-7">
                A wipe is marked incomplete until a bit-by-bit comparison verifies zero remaining signal or confirms the exact pseudo-random entropy matrix. We record the verified outcome at operation time—never retrofitted or mocked.
              </p>
            </div>

            {/* Micro verification status in Neomorphic Inset Container */}
            <div className="p-5 rounded-2xl neu-inset grid grid-cols-1 sm:grid-cols-3 gap-5 font-mono text-xs">
              <div>
                <span className="text-[#8E9DB8] block text-[10px] uppercase">Pass Criterion</span>
                <span className="text-white font-semibold text-sm">0 Bit Discrepancy</span>
              </div>
              <div>
                <span className="text-[#8E9DB8] block text-[10px] uppercase">Readback Engine</span>
                <span className="text-white font-semibold text-sm">Direct DMA IOCTL</span>
              </div>
              <div>
                <span className="text-[#8E9DB8] block text-[10px] uppercase">Audit Integrity</span>
                <span className="text-[#34D399] font-semibold text-sm flex items-center gap-1.5">
                  <CheckCircle2 className="w-4 h-4" /> Immutable
                </span>
              </div>
            </div>
          </div>

          {/* Card 2: NVMe & ATA Native Sanitize (4 cols) */}
          <div className="md:col-span-4 rounded-2xl neu-card p-8 flex flex-col justify-between">
            <div>
              <div className="w-11 h-11 rounded-xl neu-pill text-[#38BDF8] flex items-center justify-center mb-6">
                <HardDrive className="w-5 h-5 drop-shadow-[0_0_6px_rgba(56,189,248,0.5)]" />
              </div>
              <h3 className="text-lg font-sans font-semibold text-white mb-2">
                Silicon-aware flash wiping
              </h3>
              <p className="text-[#8E9DB8] text-sm leading-relaxed mb-4">
                Bypasses deceptive wear-leveling controllers using native NVMe Sanitize, ATA Secure Erase, and crypto-scramble protocols.
              </p>
            </div>
            <div className="text-xs font-mono text-[#8E9DB8] pt-4 border-t border-[#16223B]">
              Direct IOCTL / libparted integration
            </div>
          </div>

          {/* Card 3: Fragment File Carving (4 cols) */}
          <div className="md:col-span-4 rounded-2xl neu-card p-8 flex flex-col justify-between">
            <div>
              <div className="w-11 h-11 rounded-xl neu-pill text-white flex items-center justify-center mb-6">
                <SearchCode className="w-5 h-5" />
              </div>
              <h3 className="text-lg font-sans font-semibold text-white mb-2">
                Bifragment gap carving
              </h3>
              <p className="text-[#8E9DB8] text-sm leading-relaxed mb-4">
                Reconstructs fragmented images and documents across non-contiguous clusters with structure validation on headers and trailers.
              </p>
            </div>
            <div className="text-xs font-mono text-[#8E9DB8] pt-4 border-t border-[#16223B]">
              JPEG markers & PDF xref verification
            </div>
          </div>

          {/* Card 4: Write-Blocked Safety (4 cols) */}
          <div className="md:col-span-4 rounded-2xl neu-card p-8 flex flex-col justify-between">
            <div>
              <div className="w-11 h-11 rounded-xl neu-pill text-white flex items-center justify-center mb-6">
                <FileLock2 className="w-5 h-5" />
              </div>
              <h3 className="text-lg font-sans font-semibold text-white mb-2">
                Strict read-only mounting
              </h3>
              <p className="text-[#8E9DB8] text-sm leading-relaxed mb-4">
                Recovery processes operate under kernel-enforced read-only locks. Source evidence can never be modified or altered accidentally.
              </p>
            </div>
            <div className="text-xs font-mono text-[#8E9DB8] pt-4 border-t border-[#16223B]">
              Forensic write-blocking guarantee
            </div>
          </div>

          {/* Card 5: Unified Ledger & Reports (4 cols) */}
          <div className="md:col-span-4 rounded-2xl neu-card p-8 flex flex-col justify-between">
            <div>
              <div className="w-11 h-11 rounded-xl neu-pill text-[#38BDF8] flex items-center justify-center mb-6">
                <History className="w-5 h-5" />
              </div>
              <h3 className="text-lg font-sans font-semibold text-white mb-2">
                One audit trail for both
              </h3>
              <p className="text-[#8E9DB8] text-sm leading-relaxed mb-4">
                Generates signed PDF certificates alongside machine-readable JSON files with SHA-256 hashes and operator timestamps.
              </p>
            </div>
            <div className="text-xs font-mono text-[#8E9DB8] pt-4 border-t border-[#16223B]">
              Air-gapped SQLite + optional sync
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
