import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { PixLogo } from '@/components/pix-logo';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <span className="flex items-center gap-2 font-bold tracking-tight">
          <PixLogo className="size-5" />
          <span>pixcli</span>
        </span>
      ),
    },
    links: [
      {
        text: 'Documentation',
        url: '/docs',
      },
    ],
    githubUrl: 'https://github.com/pixcli/pixcli',
  };
}
