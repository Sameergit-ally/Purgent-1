'use client';

import { useEffect, useRef, useState } from 'react';

interface Node {
    x: number;
    y: number;
    vx: number;
    vy: number;
    baseX: number;
    baseY: number;
    radius: number;
    label: string;
    pulse: number;
}

interface ConstellationGridProps {
    /** Additional CSS classes on the wrapper */
    className?: string;
    /** Whether to show the dark/light toggle button (default: false) */
    showToggle?: boolean;
    /** Canvas opacity 0–1 (default: 1) */
    opacity?: number;
    /** Force dark mode on/off. When omitted the internal toggle applies. */
    forceDark?: boolean;
    /** Custom labels for the nodes */
    labels?: string[];
}

const DEFAULT_LABELS = [
    'NIST 800-88', 'DoD 5220.22-M', 'Secure Erase',
    'Forensic Recovery', 'SHA-256 Verify', 'Tamper Ledger',
    'AES-256', 'Chain of Custody', 'Bit-Level Wipe',
    'Evidence Lock', 'Audit Trail', 'Zero-Fill',
    'Crypto Shred', 'Block Verify', 'Disk Image',
    'Report Gen', 'USB Scan', 'HDD Sanitize',
];

const COLORS = {
    dark: {
        bg: 'transparent',
        node: '#38BDF8',
        nodeGlow: 'rgba(56, 189, 248, 0.35)',
        line: 'rgba(56, 189, 248, 0.12)',
        lineActive: 'rgba(56, 189, 248, 0.4)',
        text: '#8E9DB8',
        textHover: '#FFFFFF',
        hoverRing: 'rgba(56, 189, 248, 0.25)',
    },
    light: {
        bg: 'transparent',
        node: '#3B82F6',
        nodeGlow: 'rgba(59, 130, 246, 0.3)',
        line: 'rgba(59, 130, 246, 0.1)',
        lineActive: 'rgba(59, 130, 246, 0.35)',
        text: '#64748B',
        textHover: '#0B111E',
        hoverRing: 'rgba(59, 130, 246, 0.2)',
    },
};

export default function ConstellationGrid({
    className = '',
    showToggle = false,
    opacity = 1,
    forceDark,
    labels = DEFAULT_LABELS,
}: ConstellationGridProps) {
    const canvasRef = useRef<HTMLCanvasElement | null>(null);
    const [isDarkModeInternal, setIsDarkModeInternal] = useState(true);
    const isDarkMode = forceDark !== undefined ? forceDark : isDarkModeInternal;
    const animationRef = useRef<number>(0);
    const nodesRef = useRef<Node[]>([]);
    const mouseRef = useRef({ x: -1000, y: -1000 });
    const timeRef = useRef(0);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const resize = () => {
            const dpr = window.devicePixelRatio || 1;
            const rect = canvas.getBoundingClientRect();
            canvas.width = rect.width * dpr;
            canvas.height = rect.height * dpr;
            ctx.scale(dpr, dpr);
            initNodes(rect.width, rect.height);
        };

        const initNodes = (w: number, h: number) => {
            const cols = Math.ceil(Math.sqrt(labels.length * (w / h)));
            const rows = Math.ceil(labels.length / cols);
            const cellW = w / cols;
            const cellH = h / rows;
            nodesRef.current = labels.map((label, i) => {
                const col = i % cols;
                const row = Math.floor(i / cols);
                const x = cellW * col + cellW / 2 + (Math.random() - 0.5) * cellW * 0.4;
                const y = cellH * row + cellH / 2 + (Math.random() - 0.5) * cellH * 0.4;
                return {
                    x, y,
                    vx: (Math.random() - 0.5) * 0.3,
                    vy: (Math.random() - 0.5) * 0.3,
                    baseX: x, baseY: y,
                    radius: 3 + Math.random() * 2,
                    label,
                    pulse: Math.random() * Math.PI * 2,
                };
            });
        };

        const animate = () => {
            const rect = canvas.getBoundingClientRect();
            const w = rect.width;
            const h = rect.height;
            ctx.clearRect(0, 0, w, h);
            timeRef.current += 0.016;
            const colors = isDarkMode ? COLORS.dark : COLORS.light;
            const mouse = mouseRef.current;

            // Update nodes
            nodesRef.current.forEach((node) => {
                node.x += node.vx;
                node.y += node.vy;

                // Spring back to base
                const dx = node.baseX - node.x;
                const dy = node.baseY - node.y;
                node.vx += dx * 0.003;
                node.vy += dy * 0.003;
                node.vx *= 0.98;
                node.vy *= 0.98;

                // Mouse repulsion
                const mx = node.x - mouse.x;
                const my = node.y - mouse.y;
                const mDist = Math.sqrt(mx * mx + my * my);
                if (mDist < 120) {
                    const force = (120 - mDist) / 120;
                    node.vx += (mx / mDist) * force * 0.8;
                    node.vy += (my / mDist) * force * 0.8;
                }
            });

            // Draw connections
            const maxDist = 180;
            for (let i = 0; i < nodesRef.current.length; i++) {
                for (let j = i + 1; j < nodesRef.current.length; j++) {
                    const a = nodesRef.current[i];
                    const b = nodesRef.current[j];
                    const ddx = a.x - b.x;
                    const ddy = a.y - b.y;
                    const dist = Math.sqrt(ddx * ddx + ddy * ddy);
                    if (dist < maxDist) {
                        const op = 1 - dist / maxDist;
                        const aMouse = Math.sqrt((a.x - mouse.x) ** 2 + (a.y - mouse.y) ** 2);
                        const bMouse = Math.sqrt((b.x - mouse.x) ** 2 + (b.y - mouse.y) ** 2);
                        const nearMouse = aMouse < 150 || bMouse < 150;

                        ctx.beginPath();
                        ctx.moveTo(a.x, a.y);
                        ctx.lineTo(b.x, b.y);
                        ctx.strokeStyle = nearMouse ? colors.lineActive : colors.line;
                        ctx.lineWidth = nearMouse ? 1.5 : 0.8;
                        ctx.globalAlpha = op * (nearMouse ? 1 : 0.6);
                        ctx.stroke();
                        ctx.globalAlpha = 1;
                    }
                }
            }

            // Draw nodes
            nodesRef.current.forEach((node) => {
                const pulse = Math.sin(timeRef.current * 2 + node.pulse) * 0.4 + 1;
                const mDist = Math.sqrt((node.x - mouse.x) ** 2 + (node.y - mouse.y) ** 2);
                const isHovered = mDist < 60;

                // Glow
                if (isHovered) {
                    ctx.beginPath();
                    ctx.arc(node.x, node.y, 28, 0, Math.PI * 2);
                    ctx.fillStyle = colors.hoverRing;
                    ctx.fill();
                }

                // Node outer glow
                const gradient = ctx.createRadialGradient(
                    node.x, node.y, 0,
                    node.x, node.y, node.radius * 3 * pulse
                );
                gradient.addColorStop(0, colors.nodeGlow);
                gradient.addColorStop(1, 'transparent');
                ctx.beginPath();
                ctx.arc(node.x, node.y, node.radius * 3 * pulse, 0, Math.PI * 2);
                ctx.fillStyle = gradient;
                ctx.fill();

                // Node core
                ctx.beginPath();
                ctx.arc(node.x, node.y, node.radius * (isHovered ? 1.6 : 1), 0, Math.PI * 2);
                ctx.fillStyle = colors.node;
                ctx.fill();

                // Label
                if (isHovered) {
                    ctx.font = '600 11px "Inter", sans-serif';
                    ctx.fillStyle = colors.textHover;
                    ctx.textAlign = 'center';
                    ctx.fillText(node.label, node.x, node.y - 18);
                }
            });

            animationRef.current = requestAnimationFrame(animate);
        };

        const handleMouse = (e: MouseEvent) => {
            const rect = canvas.getBoundingClientRect();
            mouseRef.current = {
                x: e.clientX - rect.left,
                y: e.clientY - rect.top,
            };
        };

        const handleMouseLeave = () => {
            mouseRef.current = { x: -1000, y: -1000 };
        };

        resize();
        window.addEventListener('resize', resize);
        canvas.addEventListener('mousemove', handleMouse);
        canvas.addEventListener('mouseleave', handleMouseLeave);
        animationRef.current = requestAnimationFrame(animate);

        return () => {
            window.removeEventListener('resize', resize);
            canvas.removeEventListener('mousemove', handleMouse);
            canvas.removeEventListener('mouseleave', handleMouseLeave);
            cancelAnimationFrame(animationRef.current);
        };
    }, [isDarkMode, labels]);

    return (
        <div className={`relative w-full h-full ${className}`} style={{ opacity }}>
            {showToggle && (
                <button
                    onClick={() => setIsDarkModeInternal(!isDarkModeInternal)}
                    className="absolute top-4 right-4 z-10 px-3 py-1.5 rounded-lg text-xs font-mono tracking-wide transition-all duration-300"
                    style={{
                        background: isDarkMode ? '#16223B' : '#E2E8F0',
                        color: isDarkMode ? '#8E9DB8' : '#475569',
                        boxShadow: isDarkMode
                            ? '4px 4px 10px #070a13, -4px -4px 10px #15223b'
                            : '4px 4px 10px #CBD5E1, -4px -4px 10px #FFFFFF',
                    }}
                >
                    {isDarkMode ? '☀ Light' : '🌙 Dark'}
                </button>
            )}

            <canvas
                ref={canvasRef}
                className="w-full h-full absolute inset-0"
            />
        </div>
    );
}
