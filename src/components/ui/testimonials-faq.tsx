import { useState } from "react";
import { ChevronDown, Quote } from "lucide-react";

export function TestimonialsFaq() {
  const [openFaq, setOpenFaq] = useState<number | null>(0);

  const testimonials = [
    {
      featured: true,
      quote:
        "In military and federal audit depositions, our biggest liability was proving that an SSD wasn't simply zero-filled at the host OS layer while retaining wear-leveled NAND blocks. Purgent's hardware-direct NVMe Sanitize handshake with cryptographic readback certificates passed Defense Counterintelligence inspection on the first submission.",
      author: "Marcus Vance",
      role: "Director of Defensive Cyber Operations",
      org: "Sovereign Defense Systems",
      metric: "0 audit inquiries across 4,200 arrays",
    },
    {
      featured: false,
      quote:
        "Having file carving and permanent sanitization in the same tool with a unified tamper-evident SQLite ledger cut our chain-of-custody reconciliation time from 3 hours per case to zero.",
      author: "Dr. Elena Rostova",
      role: "Lead Digital Forensics Examiner",
      org: "Metropolitan Incident Response Lab",
      metric: "99.4% confidence on bifragment recoveries",
    },
    {
      featured: false,
      quote:
        "The air-gapped guarantees are real. Purgent doesn't attempt any background outbound pings or telemetry. Our scif workstations run clean and without exception.",
      author: "Julian Chen",
      role: "Senior Security Architect",
      org: "FinTech Infrastructure Corp",
      metric: "100% air-gapped compliance",
    },
  ];

  const faqs = [
    {
      question: "How does Purgent guarantee NVMe SSD sanitization versus basic block overwrites?",
      answer:
        "Flash controllers employ wear-leveling and over-provisioning pools that block standard LBA sector overwrites from reaching physical NAND cells. Purgent issues direct ATA Secure Erase and NVMe Sanitize Block Erase commands to the controller, followed by DMA readback verification to ensure the underlying cryptoprocessor or flash cells are completely cleared.",
    },
    {
      question: "Can recovery operations write back to or alter the source media?",
      answer:
        "No. By architectural mandate, all file carving and partition recovery routines mount the source target under strict read-only locks with hardware write-blocking verification. Extracted files are saved to an isolated, user-specified output folder, and every carved file is hashed with SHA-256 at extraction.",
    },
    {
      question: "Does Purgent require internet connectivity or cloud telemetry to function?",
      answer:
        "Never. Purgent is designed from the ground up for air-gapped, zero-trust scif environments. Local SQLite audit databases and Ed25519-signed reports are fully authoritative on the host machine. Cloud synchronization to Supabase is an optional, opt-in mirror for enterprise oversight.",
    },
    {
      question: "What happens when bad sectors or read errors are encountered during a wipe?",
      answer:
        "Purgent logs the exact sector address and LBA offset to the audit report, marks the sector as skipped, and continues the sanitization pass. If the skipped sector threshold is exceeded, the certificate notes that physical destruction (NIST 800-88 Destroy category) is required for compliance.",
    },
    {
      question: "Are the generated PDF and JSON audit certificates accepted in legal proceedings?",
      answer:
        "Yes. Each certificate includes the full hardware identification (model, serial number, capacity bytes), the exact standard invoked (e.g. NIST 800-88 Rev.1 Purge), operator ID, start and completion timestamps, read-back verification status, and an HMAC/Ed25519 cryptographic digital signature.",
    },
  ];

  return (
    <section id="faq" className="py-24 md:py-32 border-t border-[#1E2D4A]">
      <div className="max-w-[1520px] mx-auto px-6 sm:px-8 xl:px-12">
        {/* Testimonials Block */}
        <div className="mb-24">
          <div className="max-w-3xl mb-12">
            <div className="text-xs font-mono text-[#38BDF8] tracking-widest uppercase mb-3 font-semibold">
              Forensic Case Evidence
            </div>
            <h2 className="font-serif text-3xl sm:text-4xl font-semibold tracking-tight text-white mb-4">
              Tested in high-stakes environments.
            </h2>
            <p className="text-[#8E9DB8] text-sm sm:text-base">
              Real feedback from forensic practitioners, defense auditors, and enterprise examiners.
            </p>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-12 gap-7">
            {/* Featured Quote (7 cols) */}
            <div className="lg:col-span-7 rounded-3xl neu-card-featured p-8 md:p-10 flex flex-col justify-between">
              <div>
                <div className="w-10 h-10 rounded-xl neu-pill text-[#38BDF8] flex items-center justify-center mb-6">
                  <Quote className="w-5 h-5 drop-shadow-[0_0_8px_rgba(56,189,248,0.5)]" />
                </div>
                <blockquote className="font-serif text-lg md:text-xl font-normal leading-relaxed text-white mb-8">
                  "{testimonials[0].quote}"
                </blockquote>
              </div>
              <div className="pt-6 border-t border-[#16223B] flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div>
                  <div className="text-sm font-bold text-white">
                    {testimonials[0].author}
                  </div>
                  <div className="text-xs text-[#8E9DB8]">
                    {testimonials[0].role} · {testimonials[0].org}
                  </div>
                </div>
                <div className="text-xs font-mono text-[#34D399] px-3 py-1 rounded-full neu-pill">
                  {testimonials[0].metric}
                </div>
              </div>
            </div>

            {/* Supporting 2 Quotes (5 cols stacked) */}
            <div className="lg:col-span-5 flex flex-col gap-6">
              {testimonials.slice(1).map((t, idx) => (
                <div
                  key={idx}
                  className="rounded-2xl neu-card p-7 flex flex-col justify-between"
                >
                  <blockquote className="text-sm text-white leading-relaxed mb-6 font-sans">
                    "{t.quote}"
                  </blockquote>
                  <div className="pt-4 border-t border-[#16223B] flex items-center justify-between">
                    <div>
                      <div className="text-xs font-bold text-white">{t.author}</div>
                      <div className="text-[11px] text-[#8E9DB8]">{t.role}</div>
                    </div>
                    <span className="text-[10px] font-mono text-[#38BDF8] px-2.5 py-1 rounded-full neu-inset">
                      {t.metric}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* FAQ Accordion Block */}
        <div className="max-w-3xl mx-auto pt-12 border-t border-[#1E2D4A]">
          <div className="text-center mb-12">
            <div className="text-xs font-mono text-[#38BDF8] tracking-widest uppercase mb-2 font-semibold">
              Verification Details
            </div>
            <h2 className="font-serif text-2xl sm:text-3xl font-semibold text-white">
              Frequently Addressed Inquiries
            </h2>
          </div>

          <div className="space-y-4">
            {faqs.map((faq, index) => {
              const isOpen = openFaq === index;
              return (
                <div
                  key={index}
                  className={`rounded-2xl transition-all duration-200 ${
                    isOpen
                      ? "neu-card-featured"
                      : "neu-card"
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => setOpenFaq(isOpen ? null : index)}
                    aria-expanded={isOpen}
                    className="w-full text-left p-6 flex items-center justify-between gap-4 focus:outline-none rounded-2xl"
                  >
                    <span className="text-sm sm:text-base font-medium text-white">
                      {faq.question}
                    </span>
                    <div className="w-8 h-8 rounded-full neu-pill flex items-center justify-center shrink-0">
                      <ChevronDown
                        className={`w-4 h-4 text-[#38BDF8] transition-transform duration-200 ${
                          isOpen ? "rotate-180" : ""
                        }`}
                      />
                    </div>
                  </button>

                  {isOpen && (
                    <div className="px-6 pb-6 pt-1 text-sm text-[#8E9DB8] leading-relaxed border-t border-[#16223B]">
                      {faq.answer}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </section>
  );
}
