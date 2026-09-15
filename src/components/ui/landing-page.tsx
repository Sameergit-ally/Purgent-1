import { Hero } from "./hero";
import { FeaturesGrid } from "./features-grid";
import { CtaFooter } from "./cta-footer";

interface LandingPageProps {
  onOpenConsole: () => void;
}

export function LandingPage({ onOpenConsole }: LandingPageProps) {
  return (
    <div className="min-h-screen bg-[#0B111E] text-white selection:bg-[#38BDF8]/30 selection:text-white">
      <Hero onOpenConsole={onOpenConsole} />
      <FeaturesGrid />
      <CtaFooter onOpenConsole={onOpenConsole} />
    </div>
  );
}
