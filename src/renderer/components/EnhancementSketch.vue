<template>
  <div class="study-sketch" aria-hidden="true">
    <svg
      v-if="kind === 'nr'"
      viewBox="0 0 230 98"
      fill="none"
      stroke="currentColor"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <g opacity=".25" stroke-width="3">
        <path d="M9 74 30 45 46 60 64 34 87 74" />
        <path d="M10 78 86 79" />
      </g>
      <g opacity=".25" stroke-width="1">
        <path d="M8 77 29 43 48 62 65 32 90 76" />
        <path d="m20 72 9-12m7 14 8-10m16 9 7-11" />
      </g>
      <path d="M99 53q12-5 23-1m-5-5 7 5-6 5" stroke-width="1.2" opacity=".6" />
      <path
        d="m136 75 23-31 15 17 19-29 28 43M135 80q43-2 88 0"
        stroke-width="1.7"
      />
      <path
        d="m138 77 21-30 16 17 19-29 25 38"
        opacity=".3"
        stroke-width=".8"
      />
      <path
        d="m150 70 8-12m-3 17 9-12m18 11 13-25m-7 28 12-21m-3 21 9-14"
        stroke-width=".8"
        opacity=".5"
      />
      <path d="m202 16 2 7 7 2-7 2-2 7-2-7-7-2 7-2Z" stroke-width="1.2" /></svg
    ><svg
      v-else
      viewBox="0 0 230 98"
      fill="none"
      stroke="currentColor"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path
        d="M15 68Q50 18 101 28T215 33L214 79Q126 68 15 80Z"
        fill="currentColor"
        opacity=".07"
        stroke="none"
      />
      <path
        d="M16 74q29-5 52-32l28 30 40-23 26 20 24-11 27 17M15 80q103-3 200 1"
        stroke-width="1.3"
      />
      <path
        d="m26 75 14-11m-7 15 17-15m-6 14 18-16m-6 17 14-14m-4 14 11-10"
        opacity=".55"
        stroke-width="1"
      />
      <path
        d="M147 32q-3-13 9-16 14-3 17 10 3 13-10 16-13 2-16-10Z"
        fill="currentColor"
        fill-opacity=".15"
        stroke-width="1.2"
      />
      <path
        d="m159 8 1-5m20 14 5-3m-5 21 5 2m-46-18-5-3m24 32 1 5"
        opacity=".65"
      />
      <path d="M20 89q92 4 187-1" opacity=".3" />
    </svg>
  </div>
</template>
<script setup>
defineProps({ kind: { type: String, required: true } })
</script>
