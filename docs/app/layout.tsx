import { RootProvider } from 'fumadocs-ui/provider/next';
import './global.css';
import type { ReactNode } from 'react';
import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: {
    default: 'Pixcli',
    template: '%s | Pixcli',
  },
  description: 'CLI tool and Rust library for programmatic Pix payments in Brazil',
};

export default function Layout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="flex min-h-screen flex-col antialiased">
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
