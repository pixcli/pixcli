import Link from 'next/link';

const features = [
  {
    icon: '🔧',
    title: 'CLI',
    description: 'Create charges, send payments, and check balance from your terminal.',
  },
  {
    icon: '📱',
    title: 'QR Codes',
    description: 'Generate and decode Pix QR codes — static or dynamic.',
  },
  {
    icon: '🤖',
    title: 'MCP Server',
    description: 'AI agent integration out of the box with Claude Code and OpenClaw.',
  },
  {
    icon: '🔔',
    title: 'Webhooks',
    description: 'Real-time payment notifications with built-in listener server.',
  },
  {
    icon: '🦀',
    title: 'Rust Library',
    description: 'Use Pix in your own Rust projects via the pix-core and pix-efi crates.',
  },
];

export default function HomePage() {
  return (
    <main className="flex flex-col items-center justify-center px-4 py-16">
      {/* Hero */}
      <div className="text-center max-w-3xl mb-16">
        <h1 className="text-4xl font-bold mb-4 sm:text-5xl">
          Pixcli — Pix payments from the command line
        </h1>
        <p className="text-lg text-fd-muted-foreground mb-8">
          A fast, open-source CLI and Rust library for Brazilian Pix instant payments.
          Create charges, generate QR codes, manage webhooks, and integrate with AI agents.
        </p>
        <div className="flex gap-4 justify-center flex-wrap">
          <Link
            href="/docs/getting-started/installation"
            className="inline-flex items-center px-6 py-3 rounded-lg bg-fd-primary text-fd-primary-foreground font-medium hover:opacity-90 transition-opacity"
          >
            Get Started
          </Link>
          <Link
            href="https://github.com/pixcli/pixcli"
            className="inline-flex items-center px-6 py-3 rounded-lg border border-fd-border font-medium hover:bg-fd-accent transition-colors"
          >
            GitHub
          </Link>
        </div>
      </div>

      {/* Quick Start */}
      <div className="w-full max-w-2xl mb-16">
        <div className="rounded-lg border border-fd-border bg-fd-card p-6">
          <p className="text-sm text-fd-muted-foreground mb-3">Quick start</p>
          <pre className="bg-fd-secondary rounded-md p-4 overflow-x-auto text-sm">
            <code>{`# Install
cargo install pixcli

# Configure (interactive)
pixcli config init

# Generate a Pix QR code
pixcli qr generate --key "+5511999999999" --amount 25.00

# Create a charge
pixcli charge create --amount 50.00 --key "+5511999999999"

# Check balance
pixcli balance`}</code>
          </pre>
        </div>
      </div>

      {/* Feature Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6 max-w-5xl w-full">
        {features.map((feature) => (
          <div
            key={feature.title}
            className="rounded-lg border border-fd-border bg-fd-card p-6 hover:border-fd-primary/50 transition-colors"
          >
            <div className="text-3xl mb-3">{feature.icon}</div>
            <h3 className="font-semibold text-lg mb-2">{feature.title}</h3>
            <p className="text-fd-muted-foreground text-sm">{feature.description}</p>
          </div>
        ))}
      </div>
    </main>
  );
}
