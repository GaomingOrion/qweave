<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api, type Meta, type SummaryRow, type FactorBundle } from "./api";
import SummaryTable from "./components/SummaryTable.vue";
import ReturnsPanel from "./components/ReturnsPanel.vue";
import ICPanel from "./components/ICPanel.vue";

const meta = ref<Meta | null>(null);
const summary = ref<SummaryRow[]>([]);
const horizon = ref(1);
const selected = ref<string | null>(null);
const bundle = ref<FactorBundle | null>(null);
const tab = ref<"returns" | "ic">("returns");
const error = ref("");
const loading = ref(false);

onMounted(async () => {
  try {
    meta.value = await api.meta();
    summary.value = await api.summary();
    horizon.value = meta.value.horizons[0] ?? 1;
  } catch (e) {
    error.value = String(e);
  }
});

async function selectFactor(name: string) {
  if (name === selected.value) return;
  selected.value = name;
  loading.value = true;
  try {
    bundle.value = await api.factor(name);
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <header>
    <h1>qweave evaluation</h1>
    <div class="sub muted" v-if="meta">
      {{ meta.factors.length }} factors · horizons {{ meta.horizons.join(", ") }} ·
      {{ meta.quantiles }} quantiles · {{ meta.binning }} binning · {{ meta.n_days }} days
    </div>
    <div class="controls" v-if="meta">
      <label>
        horizon
        <select v-model.number="horizon">
          <option v-for="h in meta.horizons" :key="h" :value="h">{{ h }}</option>
        </select>
      </label>
      <span class="muted hint">drives the IC distribution &amp; monthly heatmap; other charts show all horizons</span>
    </div>
  </header>

  <main>
    <p v-if="error" class="error">{{ error }}</p>

    <SummaryTable
      :rows="summary"
      :horizon="horizon"
      :selected="selected"
      @select="selectFactor"
    />

    <section v-if="selected" class="detail">
      <div class="tabs">
        <button :class="{ active: tab === 'returns' }" @click="tab = 'returns'">Returns</button>
        <button :class="{ active: tab === 'ic' }" @click="tab = 'ic'">IC</button>
        <span class="muted factor-name">{{ selected }}</span>
      </div>
      <p v-if="loading" class="muted">loading…</p>
      <template v-else-if="bundle">
        <ReturnsPanel
          v-if="tab === 'returns'"
          :bundle="bundle"
          :horizon="horizon"
          :horizons="meta?.horizons ?? []"
          :weighting="meta?.weighting"
        />
        <ICPanel
          v-if="tab === 'ic'"
          :bundle="bundle"
          :horizon="horizon"
          :horizons="meta?.horizons ?? []"
        />
      </template>
    </section>
    <p v-else class="muted hint">Select a factor row to open its tearsheet.</p>
  </main>
</template>

<style scoped>
header {
  padding: 18px 24px;
  border-bottom: 1px solid var(--line);
}
.sub {
  font-size: 13px;
  margin-top: 4px;
}
.controls {
  margin-top: 10px;
  display: flex;
  gap: 12px;
  align-items: center;
}
main {
  padding: 20px 24px;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 20px;
}
.error {
  color: var(--neg);
}
.detail {
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 16px;
  background: var(--panel);
}
.tabs {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}
.tabs button.active {
  border-color: var(--accent);
  color: var(--accent);
}
.factor-name {
  margin-left: auto;
  font-variant-numeric: tabular-nums;
}
.hint {
  font-size: 13px;
}
</style>
