import { source } from '@/lib/source';
import { notFound } from 'next/navigation';
import { getMDXComponents } from '@/components/mdx';
import type { Metadata } from 'next';

export default async function Page(props: {
  params: Promise<{ slug?: string[] }>;
}) {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  const MDX = page.data.body;

  return (
    <div className="flex flex-col gap-6">
      <header className="space-y-2 border-b border-fd-border pb-6">
        <h1 className="text-3xl font-bold tracking-tight sm:text-4xl">
          {page.data.title}
        </h1>
        {page.data.description && (
          <p className="text-lg leading-relaxed text-fd-muted-foreground">
            {page.data.description}
          </p>
        )}
      </header>
      <article className="prose dark:prose-invert max-w-none">
        <MDX components={getMDXComponents()} />
      </article>
    </div>
  );
}

export async function generateStaticParams() {
  return source.generateParams();
}

export async function generateMetadata(props: {
  params: Promise<{ slug?: string[] }>;
}): Promise<Metadata> {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  return {
    title: page.data.title,
    description: page.data.description,
  };
}
