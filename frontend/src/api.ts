// Thin client over the qweave-server JSON API. Column-oriented payloads
// (`Columns`) mirror the parquet tables one-to-one; shaping happens in lib/.

export type Num = number | null;

export interface Meta {
  symbol_col: string;
  time_col: string;
  quantiles: number;
  binning: string;
  demean: string;
  weighting: string;
  horizons: number[];
  factors: string[];
  n_days: number;
  factor_count: number;
  [key: string]: unknown;
}

export type Columns = Record<string, (string | number | null)[]>;

export interface FactorBundle {
  factor: string;
  ic: Columns; // date, factor, horizon, ic, rank_ic
  quantiles: Columns; // date, factor, bin, bin_lo, bin_hi, count, mean_ret_{h}
  portfolio: Columns; // date, factor, horizon, gross, net, turnover
  monthly: Columns | null; // year, month, factor, horizon, ic_mean, rank_ic_mean
}

export type SummaryRow = Record<string, string | number | null>;

async function getJson<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error((body as { error?: string }).error ?? `${res.status} ${url}`);
  }
  return res.json() as Promise<T>;
}

async function shutdown(): Promise<void> {
  const res = await fetch("/api/shutdown", { method: "POST" });
  if (!res.ok) throw new Error(`${res.status} /api/shutdown`);
}

export const api = {
  meta: () => getJson<Meta>("/api/meta"),
  summary: () => getJson<SummaryRow[]>("/api/summary"),
  factor: (name: string) => getJson<FactorBundle>(`/api/factor/${encodeURIComponent(name)}`),
  shutdown,
};
