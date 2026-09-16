import { useState, useEffect } from "react";
import { Menu, X, Terminal } from "lucide-react";
import logoImg from "../../assets/logo.png";

interface NavbarProps {
  onOpenConsole?: () => void;
  activeTab?: string;
  onNavigateSection?: (sectionId: string) => void;
}

export function Navbar({ onOpenConsole, onNavigateSection }: NavbarProps) {
  const [scrolled, setScrolled] = useState(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setScrolled(window.scrollY > 20);
    };
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  const navLinks = [
    { label: "Capabilities", href: "#features" },
    { label: "Compliance & FAQ", href: "#faq" },
  ];

  const handleLinkClick = (e: React.MouseEvent<HTMLAnchorElement>, href: string) => {
    e.preventDefault();
    setMobileMenuOpen(false);
    if (onNavigateSection) {
      onNavigateSection(href.replace("#", ""));
    }
  };

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 transition-all duration-300 ${
        scrolled
          ? "bg-[#0B111E]/90 backdrop-blur-xl border-b border-[#1E2D4A] shadow-[0_10px_30px_rgba(7,10,19,0.8)]"
          : "bg-transparent border-b border-transparent"
      }`}
    >
      <div className="max-w-[1520px] mx-auto px-6 sm:px-8 xl:px-12 h-20 flex items-center justify-between">
        {/* Brand */}
        <a
          href="#"
          className="flex items-center gap-3.5 group text-decoration-none focus:outline-none"
        >
          <div className="w-10 h-10 rounded-xl overflow-hidden ring-1 ring-[#1E2D4A] bg-[#0B111E] flex items-center justify-center">
            <img
              src={logoImg}
              alt="Purgent logo"
              className="w-full h-full object-cover"
            />
          </div>
          <div className="flex flex-col">
            <span className="font-serif font-bold text-xl tracking-tight text-white leading-none">
              Purgent
            </span>
            <span className="text-[10px] font-mono text-[#8E9DB8] tracking-widest uppercase mt-1">
              Forensic Sanitization
            </span>
          </div>
        </a>

        {/* Center Navigation Links in Neomorphic Pill Bar */}
        <nav className="hidden md:flex items-center gap-2 px-3 py-1.5 rounded-full neu-inset">
          {navLinks.map((link) => (
            <a
              key={link.label}
              href={link.href}
              onClick={(e) => handleLinkClick(e, link.href)}
              className="px-4 py-1.5 text-xs font-sans font-medium text-[#8E9DB8] hover:text-white hover:bg-[#10192C] rounded-full transition-all"
            >
              {link.label}
            </a>
          ))}
        </nav>

        {/* Right CTA */}
        <div className="hidden md:flex items-center gap-4">
          <button
            type="button"
            onClick={onOpenConsole}
            className="px-5 py-2.5 text-xs font-sans font-semibold rounded-xl neu-btn-primary"
          >
            Launch Desktop Console
          </button>
        </div>

        {/* Mobile menu button */}
        <button
          type="button"
          aria-label={mobileMenuOpen ? "Close navigation menu" : "Open navigation menu"}
          aria-expanded={mobileMenuOpen}
          onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
          className="md:hidden p-2.5 rounded-xl neu-btn text-white focus:outline-none"
        >
          {mobileMenuOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
        </button>
      </div>

      {/* Mobile Drawer */}
      {mobileMenuOpen && (
        <div className="md:hidden border-b border-[#1E2D4A] bg-[#10192C] px-6 py-6 shadow-2xl animate-in slide-in-from-top-2 duration-150">
          <nav className="flex flex-col gap-3 mb-6">
            {navLinks.map((link) => (
              <a
                key={link.label}
                href={link.href}
                onClick={(e) => handleLinkClick(e, link.href)}
                className="text-base font-medium text-white hover:text-[#38BDF8] transition-colors py-1"
              >
                {link.label}
              </a>
            ))}
          </nav>
          <div className="flex flex-col gap-3 pt-4 border-t border-[#1E2D4A]">
            <button
              type="button"
              onClick={() => {
                setMobileMenuOpen(false);
                onOpenConsole?.();
              }}
              className="w-full flex items-center justify-center gap-2 px-4 py-3 rounded-xl neu-btn-primary font-semibold text-xs"
            >
              <Terminal className="w-4 h-4 text-white" />
              Launch Desktop Console
            </button>
          </div>
        </div>
      )}
    </header>
  );
}
