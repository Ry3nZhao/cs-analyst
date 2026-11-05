<template>
  <UApp>
    <NuxtLayout>
      <NuxtPage />
    </NuxtLayout>
    <OnboardingGlobal />
  </UApp>
</template>

<script setup lang="ts">
useHead({ htmlAttrs: { class: 'dark' } });
</script>
