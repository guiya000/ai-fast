<script setup>
import Composer from "../Composer.vue";

defineOptions({ inheritAttrs: false });
</script>

<template>
  <Composer v-bind="$attrs" />
</template>