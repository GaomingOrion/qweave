<script setup lang="ts">
import { computed, ref } from "vue";
import type { FactorBundle, Num } from "../api";
import { histogram, rollingMean, mean, median, monthLabel, isoDate, fmt, pct } from "../lib/series";
import type { ECOption } from "../lib/echarts";
import EChart from "./EChart.vue";

const props = defineProps<{ bundle: FactorBundle; horizon: number; horizons: number[] }>();

// Rank IC is the headline metric here; Pearson IC is the alternate view.
const metric = ref<"rank_ic" | "ic">("rank_ic");
const metricLabel = computed(() => (metric.value === "rank_ic" ? "Rank IC" : "IC"));

const HORIZON_COLORS = ["#2563eb", "#0891b2", "#16a34a", "#d97706", "#dc2626", "#7c3aed"];
const hColor = (i: number) => HORIZON_COLORS[i % HORIZON_COLORS.length];

const baseGrid = { left: 56, right: 18, top: 48, bottom: 44 };

/** Date-aligned values of the active metric, per horizon. */
const byHorizon = computed(() => {
  const ic = props.bundle.ic;
  const field = ic[metric.value] as Num[];
  const hcol = ic.horizon as number[];
  const dcol = ic.date as string[];
  const dates = [...new Set(dcol)].sort();
  const dateIdx = new Map(dates.map((d, i) => [d, i]));
  const byH = new Map<number, Num[]>();
  props.horizons.forEach((h) => byH.set(h, new Array<Num>(dates.length).fill(null)));
  for (let i = 0; i < field.length; i++) {
    const arr = byH.get(hcol[i]);
    const di = dateIdx.get(dcol[i]);
    if (arr && di !== undefined) arr[di] = field[i];
  }
  return { dates, byH };
});

/** Active-metric daily values for the selected horizon (for the distribution). */
const selectedSeries = computed(() => byHorizon.value.byH.get(props.horizon) ?? []);

// 1) Rolling-mean of the active metric, one line per horizon, with a zero line.
const rollingSeries = computed<ECOption>(() => {
  const { dates, byH } = byHorizon.value;
  const window = Math.min(21, Math.max(2, Math.floor(dates.length / 4)));
  const series: ECOption["series"] = props.horizons.map((h, i) => ({
    name: `h=${h}`,
    type: "line",
    showSymbol: false,
    lineStyle: { width: 1.6 },
    itemStyle: { color: hColor(i) },
    data: rollingMean(byH.get(h) ?? [], window),
    ...(i === 0
      ? {
          markLine: {
            silent: true,
            symbol: "none",
            label: { show: false },
            lineStyle: { color: "#94a3b8", type: "dashed" },
            data: [{ yAxis: 0 }],
          },
        }
      : {}),
  }));
  return {
    title: {
      text: `${metricLabel.value} · ${window}-day rolling mean by horizon`,
      left: "center",
      textStyle: { fontSize: 13 },
    },
    grid: baseGrid,
    tooltip: {
      trigger: "axis",
      valueFormatter: (v: unknown) => fmt(v, 4),
      axisPointer: { label: { formatter: (o: { value: unknown }) => isoDate(o.value) } },
    },
    legend: { top: 22, right: 8, type: "scroll" },
    xAxis: { type: "category", data: dates, boundaryGap: false, axisLabel: { formatter: (v: unknown) => monthLabel(v) } },
    yAxis: { type: "value", scale: true },
    dataZoom: [{ type: "inside" }, { type: "slider", height: 16, bottom: 8 }],
    series,
  };
});

// 2) Distribution of the active metric at the selected horizon, with summary stats.
const icHistogram = computed<ECOption>(() => {
  const vals = selectedSeries.value;
  const hist = histogram(vals, 30);
  const m = mean(vals);
  const md = median(vals);
  const finite = vals.filter((v): v is number => v !== null && Number.isFinite(v));
  const posRatio = finite.length ? finite.filter((v) => v > 0).length / finite.length : 0;
  const sub =
    finite.length > 0
      ? `mean ${fmt(m, 4)} · median ${fmt(md, 4)} · positive ${pct(posRatio, 1)} · N ${finite.length}`
      : "no data";
  return {
    title: {
      text: `${metricLabel.value} distribution (h=${props.horizon})`,
      subtext: sub,
      left: "center",
      textStyle: { fontSize: 13 },
      subtextStyle: { fontSize: 11 },
    },
    grid: { ...baseGrid, top: 56 },
    tooltip: { trigger: "axis" },
    xAxis: { type: "category", data: hist.map(([c]) => c.toFixed(3)), axisLabel: { interval: 4 } },
    yAxis: { type: "value", name: "days" },
    series: [
      {
        type: "bar",
        barCategoryGap: "0%",
        itemStyle: {
          color: (p: { name: string }) => (parseFloat(p.name) >= 0 ? "#3b82f6" : "#dc2626"),
        },
        data: hist.map(([, n]) => n),
      },
    ],
  };
});

// 3) Monthly mean IC heatmap (year × month) for the selected horizon.
const hasMonthly = computed(() => props.bundle.monthly !== null);
const monthlyField = computed(() => (metric.value === "rank_ic" ? "rank_ic_mean" : "ic_mean"));

const monthlyHeatmap = computed<ECOption>(() => {
  const m = props.bundle.monthly;
  if (!m) return {};
  const field = (m[monthlyField.value] ?? m.ic_mean) as Num[];
  const hcol = m.horizon as number[];
  const rows: number[] = [];
  hcol.forEach((h, i) => {
    if (h === props.horizon) rows.push(i);
  });
  const years = [...new Set(rows.map((i) => m.year[i] as number))].sort((a, b) => a - b);
  const yearIdx = new Map(years.map((y, i) => [y, i]));
  const months = Array.from({ length: 12 }, (_, i) => i + 1);
  let maxAbs = 1e-6;
  const data = rows.flatMap((i) => {
    const v = field[i] as Num;
    if (v === null || !Number.isFinite(v)) return [];
    maxAbs = Math.max(maxAbs, Math.abs(v));
    return [[(m.month[i] as number) - 1, yearIdx.get(m.year[i] as number) ?? 0, v]];
  });
  return {
    title: { text: `Monthly ${metricLabel.value} (h=${props.horizon})`, left: "center", textStyle: { fontSize: 13 } },
    grid: { left: 52, right: 16, top: 36, bottom: 90, containLabel: true },
    tooltip: {
      position: "top",
      formatter: (p) => {
        const d = (p as unknown as { data: [number, number, number] }).data;
        return `${years[d[1]]}-${String(months[d[0]]).padStart(2, "0")}: ${d[2].toFixed(4)}`;
      },
    },
    xAxis: { type: "category", data: months.map((mm) => String(mm)), splitArea: { show: true } },
    yAxis: { type: "category", data: years.map(String), splitArea: { show: true } },
    visualMap: {
      min: -maxAbs,
      max: maxAbs,
      calculable: true,
      orient: "horizontal",
      left: "center",
      bottom: 8,
      inRange: { color: ["#dc2626", "#f8fafc", "#2563eb"] },
    },
    series: [{ type: "heatmap", data, progressive: 0 }],
  };
});
</script>

<template>
  <div>
    <div class="metric-toggle">
      <button :class="{ active: metric === 'rank_ic' }" @click="metric = 'rank_ic'">Rank IC</button>
      <button :class="{ active: metric === 'ic' }" @click="metric = 'ic'">Pearson IC</button>
      <span class="muted hint">rolling chart shows all horizons · distribution &amp; heatmap use the selected horizon</span>
    </div>
    <div class="charts">
      <EChart :option="rollingSeries" height="360px" />
      <EChart :option="icHistogram" height="360px" />
      <EChart v-if="hasMonthly" :option="monthlyHeatmap" height="360px" />
      <p v-else class="muted note">Monthly {{ metricLabel }} heatmap unavailable for this run.</p>
    </div>
  </div>
</template>

<style scoped>
.metric-toggle {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}
.metric-toggle button.active {
  border-color: var(--accent);
  color: var(--accent);
}
.hint {
  font-size: 12px;
}
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
.note {
  align-self: center;
}
</style>
