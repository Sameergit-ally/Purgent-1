import { useState } from "react";
import { Check, Minus } from "lucide-react";

interface PricingProps {
  onOpenConsole?: () => void;
}

export function Pricing({ onOpenConsole }: PricingProps) {
  const [billingCycle, setBillingCycle] = useState<"monthly" | "yearly">("yearly");

  const tiers = [
    {
      id: "investigator",
      name: "Field Investigator",
      badge: "Single Seat",
      description: "For independent DFIR consultants and legal examiners who need offline wiping and carving.",
      priceMonthly: 89,
      priceYearly: 74,
      features: [
        { label: "NIST SP 800-88 Clear & Purge", included: true },
        { label: "DoD 5220.22-M 3-pass & 7-pass", included: true },
        { label: "Signature & structure file carving", included: true },
        { label: "PDF erasure certificates & JSON logs", included: true },
        { label: "Air-gapped local SQLite storage", included: true },
        { label: "Centralized Supabase audit sync", included: false },
        { label: "Custom cryptographic signing keys", included: false },
        { label: "Enterprise multi-operator role policies", included: false },
      ],
      cta: "Acquire License",
      recommended: false,
    },
    {
      id: "enterprise",
      name: "Enterprise DFIR",
      badge: "Most Deployed",
      description: "Complete solution for incident response teams, enterprise IT asset disposition, and forensics labs.",
      priceMonthly: 249,
      priceYearly: 199,
      features: [
        { label: "NIST SP 800-88 Clear & Purge", included: true },
        { label: "DoD 5220.22-M 3-pass & 7-pass", included: true },
        { label: "Signature & structure file carving", included: true },
        { label: "PDF erasure certificates & JSON logs", included: true },
        { label: "Air-gapped local SQLite storage", included: true },
        { label: "Centralized Supabase audit sync", included: true },
        { label: "Custom cryptographic signing keys", included: true },
        { label: "Enterprise multi-operator role policies", included: true },
      ],
      cta: "Deploy Fleet",
      recommended: true,
    },
    {
      id: "sovereign",
      name: "Sovereign / Lab",
      badge: "Dedicated Nodes",
      description: "For government, defense contractors, and specialized labs requiring source audits and HSM signing.",
      priceMonthly: 599,
      priceYearly: 499,
      features: [
        { label: "NIST SP 800-88 Clear & Purge", included: true },
        { label: "DoD 5220.22-M 3-pass & 7-pass", included: true },
        { label: "Signature & structure file carving", included: true },
        { label: "PDF erasure certificates & JSON logs", included: true },
        { label: "Air-gapped local SQLite storage", included: true },
        { label: "PKCS#11 Hardware Security Module support", included: true },
        { label: "Custom firmware sanitization profiles", included: true },
        { label: "Dedicated compliance SLA & engineer support", included: true },
      ],
      cta: "Contact Defense Sales",
      recommended: false,
    },
  ];

  return (
    <section id="pricing" className="py-24 md:py-32 border-t border-[#1E2D4A]">
      <div className="max-w-[1520px] mx-auto px-6 sm:px-8 xl:px-12">
        {/* Section Heading */}
        <div className="text-center max-w-3xl mx-auto mb-14">
          <div className="text-xs font-mono text-[#38BDF8] tracking-widest uppercase mb-3 font-semibold">
            Predictable Licensing
          </div>
          <h2 className="font-serif text-3xl sm:text-4xl font-semibold tracking-tight text-white mb-4">
            Transparent pricing for certified compliance.
          </h2>
          <p className="text-base text-[#8E9DB8] leading-relaxed mb-8">
            Every tier includes the offline Rust core engine, mandatory readback verification, and cryptographically signed PDF certificates.
          </p>

          {/* Neomorphic Inset Billing Toggle */}
          <div className="inline-flex items-center p-1.5 rounded-2xl neu-inset">
            <button
              type="button"
              onClick={() => setBillingCycle("monthly")}
              className={`px-5 py-2 rounded-xl text-xs font-semibold transition-all ${
                billingCycle === "monthly"
                  ? "neu-btn text-[#38BDF8]"
                  : "text-[#8E9DB8] hover:text-white"
              }`}
            >
              Monthly billing
            </button>
            <button
              type="button"
              onClick={() => setBillingCycle("yearly")}
              className={`px-5 py-2 rounded-xl text-xs font-semibold flex items-center gap-2 transition-all ${
                billingCycle === "yearly"
                  ? "neu-btn text-[#38BDF8]"
                  : "text-[#8E9DB8] hover:text-white"
              }`}
            >
              <span>Annual billing</span>
              <span className="px-2 py-0.5 rounded-full text-[10px] font-mono bg-[#16223B] text-[#34D399] neu-pill">
                Save 20%
              </span>
            </button>
          </div>
        </div>

        {/* Pricing Cards Grid */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-8 items-stretch">
          {tiers.map((tier) => {
            const price = billingCycle === "yearly" ? tier.priceYearly : tier.priceMonthly;
            return (
              <div
                key={tier.id}
                className={`rounded-3xl p-8 md:p-9 flex flex-col justify-between relative transition-all duration-300 ${
                  tier.recommended
                    ? "neu-card-featured lg:-translate-y-3"
                    : "neu-card hover:-translate-y-1"
                }`}
              >
                <div>
                  {/* Top line with tier name & badge */}
                  <div className="flex items-center justify-between mb-4">
                    <h3 className="font-serif text-2xl font-bold text-white">
                      {tier.name}
                    </h3>
                    <span
                      className={`px-3 py-1 rounded-full text-[10px] font-mono font-bold ${
                        tier.recommended
                          ? "bg-[#16223B] text-[#38BDF8] shadow-[0_0_10px_rgba(56,189,248,0.3)] border border-[#38BDF8]/40"
                          : "bg-[#16223B] text-[#8E9DB8] neu-pill"
                      }`}
                    >
                      {tier.badge}
                    </span>
                  </div>

                  <p className="text-xs text-[#8E9DB8] leading-relaxed mb-6">
                    {tier.description}
                  </p>

                  {/* Price */}
                  <div className="flex items-baseline gap-2 pb-6 mb-6 border-b border-[#16223B]">
                    <span className="text-4xl sm:text-5xl font-mono font-bold text-white drop-shadow-[0_2px_8px_rgba(0,0,0,0.5)]">
                      ${price}
                    </span>
                    <span className="text-xs text-[#8E9DB8] font-mono">
                      / seat / month billed {billingCycle}
                    </span>
                  </div>

                  {/* Feature List */}
                  <div className="space-y-3.5 mb-8">
                    <span className="text-[11px] font-mono text-[#38BDF8] uppercase tracking-wider block mb-3 font-semibold">
                      Included Capabilities
                    </span>
                    {tier.features.map((feature, i) => (
                      <div key={i} className="flex items-start gap-3 text-xs leading-normal">
                        {feature.included ? (
                          <div className="w-4 h-4 rounded-full neu-pill flex items-center justify-center shrink-0 mt-0.5 text-[#34D399]">
                            <Check className="w-3 h-3" />
                          </div>
                        ) : (
                          <div className="w-4 h-4 rounded-full neu-inset flex items-center justify-center shrink-0 mt-0.5 text-[#8E9DB8]/40">
                            <Minus className="w-3 h-3" />
                          </div>
                        )}
                        <span className={feature.included ? "text-white font-medium" : "text-[#8E9DB8]/40"}>
                          {feature.label}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>

                {/* Card CTA button */}
                <button
                  type="button"
                  onClick={onOpenConsole}
                  className={`w-full py-3.5 px-4 font-semibold text-xs tracking-wide transition-all ${
                    tier.recommended
                      ? "neu-btn-primary"
                      : "neu-btn hover:text-[#38BDF8]"
                  }`}
                >
                  {tier.cta}
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
