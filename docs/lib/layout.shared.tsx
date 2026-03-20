import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: '💲 Pixcli',
    },
    links: [
      {
        text: 'Docs',
        url: '/docs',
      },
    ],
    githubUrl: 'https://github.com/pixcli/pixcli',
  };
}
