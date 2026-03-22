import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <span className="flex items-center gap-1.5 font-bold tracking-tight">
          <span className="text-lg">💲</span>
          <span>Pixcli</span>
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
