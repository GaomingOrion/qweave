<script setup lang="ts">
import { computed } from "vue";
import type { FactorBundle, Num } from "../api";
import { cumsum, drawdown, pct, monthLabel, isoDate } from "../lib/series";
import type { ECOption } from "../lib/echarts";
import EChart from "./EChart.vue";

const props = defineProps<{
  bundle: FactorBundle;
  horizon: number;
  horizons: number[];
  weighting?: string;
}>();

// Q1..Qn cold→warm, so bucket colour is consistent across every quantile chart.
const PALETTE = [
  "#3b82f6", "#22d3ee", "#10b981", "#84cc16", "#eab308",
  "#f59e0b", "#f97316", "#ef4444", "#ec4899", "#a855f7",
];
const binColor = (i: number, n: number) =>
  PALETTE[Math.round((i / Math.max(1, n - 1)) * (PALETTE.length - 1))];
// Distinct hues per horizon for the multi-horizon overlays.
const HORIZON_COLORS = ["#2563eb", "#0891b2", "#16a34a", "#d97706", "#dc2626", "#7c3aed"];
const hColor = (i: number) => HORIZON_COLORS[i % HORIZON_COLORS.length];

const bins = computed(() =>
  [...new Set((props.bundle.quantiles.bin as number[]).map(Number))].sort((a, b) => a - b),
);

// Correct daily-cumulative basis is the 1-bar forward return; longer horizons
// overlap and cannot be summed day-by-day. Fall back to the shortest horizon.
const cumHorizon = computed(() =>
  props.horizons.includes(1) ? 1 : Math.min(...props.horizons),
);

/** Full-period count-weighted mean return per bin, for one horizon. */
function binMean(h: number): Map<number, number> {
  const q = props.bundle.quantiles;
  const ret = q[`mean_ret_${h}`] as Num[] | undefined;
  const binArr = q.bin as number[];
  const cnt = q.count as number[];
  const acc = new Map<number, { s: number; w: number }>();
  if (!ret) return new Map();
  for (let i = 0; i < binArr.length; i++) {
    const r = ret[i];
    const w = cnt[i];
    if (r === null || !Number.isFinite(r) || !w) continue;
    const e = acc.get(binArr[i]) ?? { s: 0, w: 0 };
    e.s += r * w;
    e.w += w;
    acc.set(binArr[i], e);
  }
  const out = new Map<number, number>();
  acc.forEach((v, k) => out.set(k, v.w > 0 ? v.s / v.w : NaN));
  return out;
}

const baseGrid = { left: 56, right: 18, top: 48, bottom: 44, containLabel: true };
const pctTip = { trigger: "axis", valueFormatter: (v: unknown) => pct(v, 3) } as const;
const dateLabel = { formatter: (v: unknown) => monthLabel(v) };
const datePointer = { label: { formatter: (o: { value: unknown }) => isoDate(o.value) } };

// 1) Mean return by quantile, one bar series per horizon.
const quantileBar = computed<ECOption>(() => ({
  title: {
    text: "Mean forward return by quantile (all horizons)",
    left: "center",
    textStyle: { fontSize: 13 },
  },
  grid: baseGrid,
  tooltip: pctTip,
  legend: { top: 22, right: 8, type: "scroll" },
  xAxis: { type: "category", data: bins.value.map((b) => `Q${b}`) },
  yAxis: { type: "value", scale: true, axisLabel: { formatter: (v: number) => pct(v, 1) } },
  series: props.horizons.map((h, i) => {
    const m = binMean(h);
    return {
      name: `h=${h}`,
      type: "bar",
      itemStyle: { color: hColor(i) },
      data: bins.value.map((b) => m.get(b) ?? null),
    };
  }),
}));

// 2) Top-minus-bottom spread by horizon, with the per-day-equivalent overlay so
//    longer horizons (which accumulate a larger raw spread) stay comparable.
const spreadByHorizon = computed<ECOption>(() => {
  const top = bins.value[bins.value.length - 1];
  const bot = bins.value[0];
  const spread = props.horizons.map((h) => {
    const m = binMean(h);
    const t = m.get(top);
    const b = m.get(bot);
    return t !== undefined && b !== undefined ? t - b : null;
  });
  const perDay = spread.map((s, i) => (s === null ? null : s / props.horizons[i]));
  return {
    title: { text: "Top − bottom spread by horizon", left: "center", textStyle: { fontSize: 13 } },
    grid: baseGrid,
    tooltip: pctTip,
    legend: { top: 22, right: 8 },
    xAxis: { type: "category", data: props.horizons.map((h) => `h=${h}`) },
    yAxis: [
      { type: "value", scale: true, name: "h-day", axisLabel: { formatter: (v: number) => pct(v, 1) } },
      { type: "value", scale: true, name: "per-day", position: "right", axisLabel: { formatter: (v: number) => pct(v, 2) } },
    ],
    series: [
      {
        name: "h-day spread",
        type: "bar",
        data: spread,
        itemStyle: {
          color: (p) => (typeof p.data === "number" && p.data >= 0 ? "#2563eb" : "#dc2626"),
        },
      },
      {
        name: "per-day equivalent",
        type: "line",
        yAxisIndex: 1,
        symbol: "circle",
        symbolSize: 6,
        itemStyle: { color: "#f59e0b" },
        data: perDay,
      },
    ],
  };
});

// 3) Cumulative return by quantile — pinned to the h=1 daily basis (independent
//    of the global horizon selector, which would otherwise sum overlapping
//    forward returns and overstate the curve).
const cumulativeByQuantile = computed<ECOption>(() => {
  const q = props.bundle.quantiles;
  const h = cumHorizon.value;
  const dates = [...new Set(q.date as string[])].sort();
  const dateIdx = new Map(dates.map((d, i) => [d, i]));
  const ret = (q[`mean_ret_${h}`] as Num[]) ?? [];
  const binArr = q.bin as number[];
  const dateArr = q.date as string[];
  const perBin = new Map<number, Num[]>();
  bins.value.forEach((b) => perBin.set(b, new Array(dates.length).fill(null)));
  for (let i = 0; i < binArr.length; i++) {
    const arr = perBin.get(binArr[i]);
    const di = dateIdx.get(dateArr[i]);
    if (arr && di !== undefined) arr[di] = ret[i];
  }
  const n = bins.value.length;
  return {
    title: {
      text: `Cumulative return by quantile (h=${h}, daily rebalance)`,
      left: "center",
      textStyle: { fontSize: 13 },
    },
    grid: baseGrid,
    tooltip: { ...pctTip, axisPointer: datePointer },
    legend: { top: 22, right: 8, type: "scroll" },
    xAxis: { type: "category", data: dates, boundaryGap: false, axisLabel: dateLabel },
    yAxis: { type: "value", scale: true, axisLabel: { formatter: (v: number) => pct(v, 0) } },
    dataZoom: [{ type: "inside" }, { type: "slider", height: 16, bottom: 8 }],
    series: bins.value.map((b, i) => ({
      name: `Q${b}`,
      type: "line",
      showSymbol: false,
      lineStyle: { width: 1.4 },
      itemStyle: { color: binColor(i, n) },
      data: cumsum(perBin.get(b) ?? []),
    })),
  };
});

// 4) Long-short net value + underwater drawdown, one sleeve per horizon.
const longShort = computed<ECOption>(() => {
  const p = props.bundle.portfolio;
  const horizonCol = p.horizon as number[];
  const dateArr = p.date as string[];
  const dates = [...new Set(dateArr)].sort();
  const dateIdx = new Map(dates.map((d, i) => [d, i]));
  const series: ECOption["series"] = [];
  props.horizons.forEach((h, hi) => {
    const net = new Array<Num>(dates.length).fill(null);
    for (let i = 0; i < horizonCol.length; i++) {
      if (horizonCol[i] === h) {
        const di = dateIdx.get(dateArr[i]);
        if (di !== undefined) net[di] = p.net[i] as Num;
      }
    }
    const cum = cumsum(net);
    const color = hColor(hi);
    // Same series name → one legend entry toggles both net line and drawdown.
    series.push({
      name: `h=${h}`,
      type: "line",
      xAxisIndex: 0,
      yAxisIndex: 0,
      showSymbol: false,
      lineStyle: { width: 1.4 },
      itemStyle: { color },
      data: cum,
    });
    series.push({
      name: `h=${h}`,
      type: "line",
      xAxisIndex: 1,
      yAxisIndex: 1,
      showSymbol: false,
      lineStyle: { width: 1 },
      itemStyle: { color },
      data: drawdown(cum),
    });
  });
  const weightNote = props.weighting ? ` · ${props.weighting}-weighted` : "";
  return {
    title: {
      text: `Long-short net value by horizon${weightNote}`,
      left: "center",
      textStyle: { fontSize: 13 },
    },
    grid: [
      { left: 56, right: 18, top: 48, height: "48%" },
      { left: 56, right: 18, top: "72%", height: "18%" },
    ],
    tooltip: { trigger: "axis", valueFormatter: (v: unknown) => pct(v, 2), axisPointer: datePointer },
    legend: { top: 22, right: 8, data: props.horizons.map((h) => `h=${h}`) },
    xAxis: [
      { type: "category", gridIndex: 0, data: dates, boundaryGap: false, axisLabel: { show: false } },
      { type: "category", gridIndex: 1, data: dates, boundaryGap: false, axisLabel: dateLabel },
    ],
    yAxis: [
      { type: "value", gridIndex: 0, scale: true, name: "net", axisLabel: { formatter: (v: number) => pct(v, 0) } },
      { type: "value", gridIndex: 1, scale: true, name: "drawdown", axisLabel: { formatter: (v: number) => pct(v, 0) } },
    ],
    dataZoom: [
      { type: "inside", xAxisIndex: [0, 1] },
      { type: "slider", xAxisIndex: [0, 1], height: 14, bottom: 6 },
    ],
    series,
  };
});
</script>

<template>
  <div class="charts">
    <EChart :option="quantileBar" height="340px" />
    <EChart :option="spreadByHorizon" height="340px" />
    <EChart :option="cumulativeByQuantile" height="380px" />
    <EChart :option="longShort" height="420px" />
  </div>
</template>

<style scoped>
.charts {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 18px;
}
@media (max-width: 900px) {
  .charts {
    grid-template-columns: 1fr;
  }
}
</style>
