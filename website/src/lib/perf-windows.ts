// The published Windows run (site/windows/data.json, written by
// scripts/build_site.py --platform windows): its own metrics, its own page,
// never ranked against macOS's.
import raw from '@/generated/perf-windows.json';
import { fmt, type Stat } from '@/lib/perf';

type Format = { title: string; unit: string; digits: number; scale: number; source: string; applies: string };
type Cell = { framework: string; mode: string } & Record<string, Stat | number | string>;
type Scene = {
  scene: string;
  run: string;
  started_utc: string;
  frameworks: string[];
  modes: string[];
  trials: number;
  protocol: Record<string, any>;
  environment: Record<string, any>;
  toolchains: Record<string, any>;
  cells: Cell[];
};
type Windows = {
  available: boolean;
  verdict: string;
  commit: string;
  frameworks: Record<string, string>;
  metrics: Record<string, { title: string; unit: string; source: string }>;
  metricFormats: Record<string, Format>;
  modes: Record<string, string>;
  sceneText: Record<string, { title: string; note: string }>;
  scenes: Scene[];
  worst_ratios?: Record<string, { ratio: number; scene: string; mode: string; rival: string }>;
};

const w = raw as unknown as Windows;
export const available = Boolean(w.available);
export const windows = w;
export const repo = 'https://github.com/zeronsh/zgx';

export const scene = (name: string) => w.scenes.find((s) => s.scene === name)!;
const cell = (s: Scene, framework: string, mode: string) => s.cells.find((c) => c.framework === framework && c.mode === mode);
export function value(s: Scene, framework: string, mode: string, key: string) {
  return (cell(s, framework, mode)?.[key] as Stat | undefined)?.median;
}

/** A metric's format: the builder's WINDOWS_METRICS where it has one, else the data's own title and unit. */
export function format(metric: string): Format {
  const own = w.metrics[metric];
  return w.metricFormats?.[metric] ?? { ...own, digits: 1, scale: 1, applies: 'all' };
}

/** One metric of one scene, by mode, as the macOS charts have it. */
export function chart(s: Scene, metric: string) {
  const m = format(metric);
  const modes = s.modes.filter((mode) => m.applies === 'all' || mode !== 'idle');
  const groups = modes.map((mode) => ({
    mode,
    label: w.modes[mode] ?? mode,
    bars: s.frameworks.flatMap((f) => {
      const stat = cell(s, f, mode)?.[m.source] as Stat | undefined;
      if (!stat) return [];
      return [{ framework: f, name: w.frameworks[f] ?? f, median: stat.median * m.scale, min: stat.min * m.scale, max: stat.max * m.scale, places: m.digits }];
    }),
  }));
  const top = Math.max(...groups.flatMap((g) => g.bars.map((b) => b.median))) || 1;
  return { title: w.metrics[metric]?.title ?? m.title, unit: w.metrics[metric]?.unit ?? m.unit, groups, top };
}

/** Where zgx is furthest behind the best rival in each metric, worded as the builder words it. */
export function worstRatios() {
  return Object.entries(w.worst_ratios ?? {}).map(([metric, r]) => {
    const m = format(metric);
    const s = scene(r.scene);
    const shown = (f: string) => fmt((value(s, f, r.mode, m.source) ?? 0) * m.scale, m.digits);
    return {
      metric: w.metrics[metric]?.title ?? metric,
      ratio: r.ratio,
      where: `${(w.sceneText[r.scene]?.title ?? r.scene).toLowerCase()}, ${(w.modes[r.mode] ?? r.mode).toLowerCase()}`,
      ours: shown('zgx'),
      theirs: shown(r.rival),
      rival: w.frameworks[r.rival] ?? r.rival,
    };
  });
}
