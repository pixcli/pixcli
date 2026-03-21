import Link from 'next/link';
import {
  Terminal,
  QrCode,
  Bot,
  Bell,
  Cog,
} from 'lucide-react';
import type { ReactNode } from 'react';

const features: { icon: ReactNode; title: string; description: string; href: string }[] = [
  {
    icon: <Terminal className="size-5" />,
    title: 'CLI',
    description: 'Create charges, send payments, and check balance — all from your terminal.',
    href: '/docs/cli/overview',
  },
  {
    icon: <QrCode className="size-5" />,
    title: 'QR Codes',
    description: 'Generate and decode Pix QR codes, static or dynamic, in seconds.',
    href: '/docs/getting-started/first-qr-code',
  },
  {
    icon: <Bot className="size-5" />,
    title: 'MCP Server',
    description: 'AI agent integration out of the box with Claude Code and OpenClaw.',
    href: '/docs/mcp/overview',
  },
  {
    icon: <Bell className="size-5" />,
    title: 'Webhooks',
    description: 'Real-time payment notifications with a built-in listener server.',
    href: '/docs/webhooks/setup',
  },
  {
    icon: <Cog className="size-5" />,
    title: 'Rust Library',
    description: 'Use Pix in your own Rust projects via the pix-core and pix-efi crates.',
    href: '/docs/api/pix-core',
  },
];

export default function HomePage() {
  return (
    <main className="relative flex flex-col items-center justify-center overflow-hidden">
      {/* Background pattern */}
      <div className="dot-pattern pointer-events-none absolute inset-0 text-fd-foreground" />

      {/* Hero */}
      <section className="relative z-10 w-full max-w-4xl px-6 pt-24 pb-20 text-center sm:pt-32 sm:pb-28">
        <div className="animate-fade-in-up mb-6">
          <span className="badge border border-fd-border text-fd-muted-foreground">
            Open Source &middot; MIT Licensed
          </span>
        </div>

        <h1 className="animate-fade-in-up animation-delay-100 text-4xl font-extrabold tracking-tight sm:text-5xl lg:text-6xl">
          Pix payments from{' '}
          <span className="hero-gradient">the command line</span>
        </h1>

        <p className="animate-fade-in-up animation-delay-200 mx-auto mt-6 max-w-2xl text-lg leading-relaxed text-fd-muted-foreground sm:text-xl">
          A fast, open-source CLI and Rust library for Brazilian Pix instant
          payments. Create charges, generate QR codes, manage webhooks, and
          integrate with AI agents.
        </p>

        <div className="animate-fade-in-up animation-delay-300 mt-10 flex flex-wrap justify-center gap-4">
          <Link
            href="/docs/getting-started/installation"
            className="glow-green inline-flex items-center gap-2 rounded-lg bg-fd-primary px-7 py-3 font-semibold text-fd-primary-foreground hover:opacity-90"
          >
            Get Started
            <span aria-hidden="true">&rarr;</span>
          </Link>
          <Link
            href="https://github.com/pixcli/pixcli"
            className="inline-flex items-center gap-2 rounded-lg border border-fd-border px-7 py-3 font-semibold hover:bg-fd-accent"
          >
            <svg className="size-4" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 0C5.37 0 0 5.37 0 12c0 5.3 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61-.546-1.385-1.335-1.755-1.335-1.755-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.605-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 21.795 24 17.295 24 12 24 5.37 18.63 0 12 0z" />
            </svg>
            GitHub
          </Link>
        </div>
      </section>

      {/* Quick Start */}
      <section className="relative z-10 w-full max-w-2xl px-6 pb-20">
        <div className="glow-green rounded-xl border border-fd-border bg-fd-card p-6 sm:p-8">
          <div className="mb-4 flex items-center gap-2">
            <div className="flex gap-1.5">
              <div className="size-3 rounded-full bg-fd-muted-foreground/20" />
              <div className="size-3 rounded-full bg-fd-muted-foreground/20" />
              <div className="size-3 rounded-full bg-fd-muted-foreground/20" />
            </div>
            <span className="ml-2 text-xs font-medium tracking-wide uppercase text-fd-muted-foreground">
              Quick Start
            </span>
          </div>
          <pre className="quick-start-code overflow-x-auto rounded-lg bg-fd-secondary p-4 sm:p-5">
            <code>{`$ cargo install pixcli

$ pixcli config init
  → Provider, credentials, certificate...

$ pixcli qr generate \\
    --key "+5511999999999" \\
    --amount 25.00

$ pixcli charge create \\
    --amount 50.00 \\
    --key "+5511999999999"

$ pixcli balance`}</code>
          </pre>
        </div>
      </section>

      {/* Feature Cards */}
      <section className="relative z-10 w-full max-w-5xl px-6 pb-24">
        <div className="mb-10 text-center">
          <h2 className="text-2xl font-bold tracking-tight sm:text-3xl">
            Everything you need for Pix
          </h2>
          <p className="mt-3 text-fd-muted-foreground">
            One toolkit — from quick QR codes to full payment automation.
          </p>
        </div>

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((feature) => (
            <Link
              key={feature.title}
              href={feature.href}
              className="card-lift group rounded-xl border border-fd-border bg-fd-card p-6 no-underline hover:border-fd-primary/50"
            >
              <div className="mb-4 inline-flex rounded-lg border border-fd-border bg-fd-secondary p-2.5 text-fd-primary">
                {feature.icon}
              </div>
              <h3 className="text-base font-semibold">{feature.title}</h3>
              <p className="mt-1.5 text-sm leading-relaxed text-fd-muted-foreground">
                {feature.description}
              </p>
            </Link>
          ))}
        </div>
      </section>
    </main>
  );
}
