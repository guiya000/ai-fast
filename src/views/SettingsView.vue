<script setup>
import { computed, nextTick, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import {
  NButton,
  NCard,
  NCheckbox,
  NCollapse,
  NCollapseItem,
  NDivider,
  NEmpty,
  NForm,
  NFormItem,
  NGrid,
  NGridItem,
  NIcon,
  NInput,
  NInputNumber,
  NLayout,
  NLayoutContent,
  NModal,
  NPageHeader,
  NPopconfirm,
  NSelect,
  NSpace,
  NSwitch,
  NTable,
  NTag,
  useMessage,
} from "naive-ui";
import ChevronLeftIcon from "../components/icons/ChevronLeftIcon.vue";
import PlusIcon from "../components/icons/PlusIcon.vue";
import TrashIcon from "../components/icons/TrashIcon.vue";
import { copyFor } from "../i18n";

const router = useRouter();
const message = useMessage();
const locale = ref("zh-CN");
const copy = computed(() => copyFor(locale.value));
const loading = ref(true);
const saving = ref(false);
const activeModelId = ref(null);
const theme = ref("system");
const themeClass = computed(() => `theme-${theme.value || "system"}`);
const autostart = ref(false);
const suppliers = ref([]);
const editingIndex = ref(null);
const editingSupplier = ref(null);
const expandedModelNames = ref([]);
const supplierFormRef = ref(null);
const addModelMethodOpen = ref(false);
const modelQuery = ref({
  open: false,
  loading: false,
  apiKey: "",
  search: "",
  items: [],
  selectedIds: [],
  targetModel: null,
});

function requiredRule(message, trigger = ["input", "blur"]) {
  return { required: true, message, trigger };
}

function validateTrimmedLength(value, message) {
  const text = String(value ?? "").trim();
  if (!text) return true;
  if (text.length < 2 || text.length > 20) return new Error(message);
  return true;
}

const parameterTypes = ["string", "boolean", "number"];
const functionTypes = ["chat", "image"];

function normalizeParamValue(value, valueType) {
  if (valueType === "boolean") return value === true || value === "true";
  if (valueType === "number") return value === null || value === undefined || value === "" ? null : Number(value);
  return value === null || value === undefined ? "" : String(value);
}

function normalizeParamOptions(options, valueType) {
  if (valueType !== "string") return [];
  const values = Array.isArray(options) ? options : String(options || "").split(/[\n,]/);
  return [...new Set(values.map((option) => String(option ?? "").trim()).filter(Boolean))];
}

function createParam() {
  return {
    key: "",
    description: "",
    valueType: "string",
    value: "",
    options: [],
  };
}

function normalizeModelParams(params = []) {
  return params.map((param) => {
    const valueType = parameterTypes.includes(param.valueType) ? param.valueType : "string";
    const options = normalizeParamOptions(param.options, valueType);
    const value = normalizeParamValue(param.value, valueType);
    return {
      key: String(param.key ?? "").trim(),
      description: String(param.description ?? "").trim(),
      valueType,
      value: valueType === "string" && options.length && !options.includes(value) ? options[0] : value,
      options,
    };
  });
}

const supplierNameRules = computed(() => [
  requiredRule(copy.value.fieldRequired),
  {
    validator: (_, value) => {
      const lengthError = validateTrimmedLength(value, copy.value.nameLengthInvalid);
      if (lengthError !== true) return lengthError;
      const name = String(value ?? "").trim().toLocaleLowerCase();
      const duplicate = suppliers.value.some((item, index) => index !== editingIndex.value
        && item.name.trim().toLocaleLowerCase() === name);
      return duplicate ? new Error(copy.value.duplicateSupplierName) : true;
    },
    trigger: ["input", "blur"],
  },
]);

const baseUrlRules = computed(() => [
  requiredRule(copy.value.fieldRequired),
]);

function modelNameRules(modelIndex) {
  return [
    requiredRule(copy.value.fieldRequired),
    {
      validator: (_, value) => {
        const lengthError = validateTrimmedLength(value, copy.value.nameLengthInvalid);
        if (lengthError !== true) return lengthError;
        const name = String(value ?? "").trim().toLocaleLowerCase();
        const duplicate = editingSupplier.value?.models.some((model, index) => index !== modelIndex
          && model.name.trim().toLocaleLowerCase() === name);
        return duplicate ? new Error(copy.value.duplicateModelName) : true;
      },
      trigger: ["input", "blur"],
    },
  ];
}

const modelIdRules = computed(() => [requiredRule(copy.value.fieldRequired)]);
const apiFormatRules = computed(() => [requiredRule(copy.value.fieldRequired, "change")]);
const apiKeyRules = computed(() => [requiredRule(copy.value.fieldRequired)]);
const parameterTypeOptions = computed(() => [
  { label: copy.value.paramString, value: "string" },
  { label: copy.value.paramBoolean, value: "boolean" },
  { label: copy.value.paramNumber, value: "number" },
]);
const functionTypeOptions = computed(() => [
  { label: copy.value.chatFunction, value: "chat" },
  { label: copy.value.imageFunction, value: "image" },
]);
const apiFormatOptions = computed(() => [
  { label: copy.value.chatCompletions, endpoint: copy.value.chatCompletionsEndpoint, value: "chatCompletions" },
  { label: copy.value.responsesApi, endpoint: copy.value.responsesEndpoint, value: "responses" },
  { label: copy.value.anthropicMessages, endpoint: copy.value.anthropicMessagesEndpoint, value: "anthropicMessages" },
  { label: copy.value.imagesApi, endpoint: copy.value.imagesEndpoint, value: "images" },
]);

function apiFormatEndpoint(apiFormat) {
  return apiFormatOptions.value.find((option) => option.value === apiFormat)?.endpoint || "";
}

const supplierRules = computed(() => {
  const rules = {
    name: supplierNameRules.value,
    config: {
      baseURL: baseUrlRules.value,
    },
    models: [],
  };
  editingSupplier.value?.models.forEach((_, modelIndex) => {
    rules.models[modelIndex] = {
      name: modelNameRules(modelIndex),
      id: modelIdRules.value,
      apiFormat: apiFormatRules.value,
      apiKey: apiKeyRules.value,
    };
  });
  return rules;
});

function createId() {
  return globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function createModel() {
  return {
    id: "",
    localId: createId(),
    name: copy.value.modelDefaultName,
    functionType: "chat",
    apiFormat: "chatCompletions",
    requestPath: "",
    apiKey: "",
    params: [],
  };
}

const filteredQueriedModels = computed(() => {
  const keyword = modelQuery.value.search.trim().toLocaleLowerCase();
  if (!keyword) return modelQuery.value.items;
  return modelQuery.value.items.filter((item) => [item.name, item.id, item.ownedBy]
    .some((value) => String(value || "").toLocaleLowerCase().includes(keyword)));
});
const queriedModelCount = computed(() => modelQuery.value.items.length);
const selectedQueriedModelCount = computed(() => modelQuery.value.selectedIds.length);
const visibleQueriedModelCount = computed(() => filteredQueriedModels.value.length);
const allQueriedModelsSelected = computed(() => visibleQueriedModelCount.value > 0
  && filteredQueriedModels.value.every((item) => modelQuery.value.selectedIds.includes(item.id)));
const someQueriedModelsSelected = computed(() => filteredQueriedModels.value.some((item) => modelQuery.value.selectedIds.includes(item.id))
  && !allQueriedModelsSelected.value);

function closeModelQuery() {
  if (modelQuery.value.loading) return;
  modelQuery.value = {
    open: false,
    loading: false,
    apiKey: "",
    search: "",
    items: [],
    selectedIds: [],
    targetModel: null,
  };
}

function openModelQuery() {
  if (!editingSupplier.value || modelQuery.value.loading) return;
  modelQuery.value = {
    open: true,
    loading: false,
    apiKey: "",
    search: "",
    items: [],
    selectedIds: [],
    targetModel: createModel(),
  };
}

async function queryProviderModels(model) {
  if (modelQuery.value.loading) return;
  const baseUrl = editingSupplier.value?.config?.baseURL?.trim() || "";
  const apiKey = modelQuery.value.apiKey.trim();
  if (!baseUrl) {
    message.error(copy.value.modelListBaseUrlRequired, { duration: 2000 });
    return;
  }
  if (!apiKey) {
    message.error(copy.value.modelListApiKeyRequired, { duration: 2000 });
    return;
  }
  model.apiKey = apiKey;
  modelQuery.value = {
    open: true,
    loading: true,
    apiKey,
    search: "",
    items: [],
    selectedIds: [],
    targetModel: model,
  };
  try {
    const items = await invoke("list_provider_models", {
      request: { baseUrl, apiKey },
    });
    modelQuery.value = {
      open: true,
      loading: false,
      apiKey,
      search: "",
      items: Array.isArray(items) ? items : [],
      selectedIds: [],
      targetModel: model,
    };
    if (!modelQuery.value.items.length) {
      message.warning(copy.value.modelListEmpty, { duration: 2000 });
    }
  } catch (error) {
    modelQuery.value = {
      open: false,
      loading: false,
      apiKey: "",
      search: "",
      items: [],
      selectedIds: [],
      targetModel: null,
    };
    message.error(`${copy.value.modelListQueryFailed}：${String(error)}`, { duration: 3000 });
  }
}

function toggleQueriedModel(id, checked) {
  const selectedIds = new Set(modelQuery.value.selectedIds);
  if (checked) selectedIds.add(id);
  else selectedIds.delete(id);
  modelQuery.value.selectedIds = [...selectedIds];
}

function toggleAllQueriedModels(checked) {
  const selectedIds = new Set(modelQuery.value.selectedIds);
  filteredQueriedModels.value.forEach((item) => {
    if (checked) selectedIds.add(item.id);
    else selectedIds.delete(item.id);
  });
  modelQuery.value.selectedIds = [...selectedIds];
}

function importedModelName(item) {
  const name = String(item.name || item.id || "").trim();
  return name.length <= 20 ? name : String(item.id || name).trim().slice(0, 20);
}

function uniqueModelName(name, usedNames) {
  const normalizedName = String(name || copy.value.modelDefaultName).trim().slice(0, 20);
  let candidate = normalizedName || copy.value.modelDefaultName;
  let suffix = 2;
  while (usedNames.has(candidate.toLocaleLowerCase())) {
    const suffixText = ` (${suffix})`;
    candidate = `${normalizedName.slice(0, 20 - suffixText.length)}${suffixText}`;
    suffix += 1;
  }
  usedNames.add(candidate.toLocaleLowerCase());
  return candidate;
}

function importQueriedModels() {
  const targetModel = modelQuery.value.targetModel;
  const selectedItems = modelQuery.value.items.filter((item) => modelQuery.value.selectedIds.includes(item.id));
  if (!targetModel || !selectedItems.length || !editingSupplier.value) return;
  const usedNames = new Set(editingSupplier.value.models
    .filter((model) => model !== targetModel)
    .map((model) => model.name.trim().toLocaleLowerCase()));
  const usePlaceholder = editingSupplier.value.models.includes(targetModel)
    && !targetModel.id.trim()
    && targetModel.name === copy.value.modelDefaultName;
  selectedItems.forEach((item, index) => {
    const imported = {
      ...createModel(),
      name: uniqueModelName(importedModelName(item), usedNames),
      id: item.id,
      functionType: targetModel.functionType,
      apiFormat: targetModel.apiFormat,
      requestPath: targetModel.requestPath,
      apiKey: targetModel.apiKey,
      params: normalizeModelParams(targetModel.params),
    };
    if (usePlaceholder && index === 0) {
      Object.assign(targetModel, imported, { localId: targetModel.localId });
    } else {
      editingSupplier.value.models.push(imported);
    }
  });
  const importedCount = selectedItems.length;
  closeModelQuery();
  message.success(`${copy.value.modelListImported}：${importedCount}`);
}

function createSupplier() {
  return {
    name: copy.value.supplierDefaultName,
    config: { officialAddress: "", baseURL: "" },
    models: [],
  };
}

function cloneSupplier(supplier) {
  return {
    name: supplier.name || "",
    config: {
      officialAddress: supplier.config?.officialAddress || "",
      baseURL: supplier.config?.baseURL || supplier.config?.baseUrl || "",
    },
    models: (supplier.models || []).map((model) => ({
      ...model,
      localId: model.localId || createId(),
      functionType: functionTypes.includes(model.functionType)
        ? model.functionType
        : (model.apiFormat === "images" || model.apiFormat === "imagesApi" ? "image" : "chat"),
      apiFormat: model.apiFormat || "chatCompletions",
      requestPath: model.requestPath || "",
      apiKey: model.apiKey || "",
      apiKeyVisible: false,
      params: normalizeModelParams(model.params),
    })),
  };
}

function modelConfigId(supplierName, modelName) {
  return `${supplierName}::${modelName}`;
}

function openSupplier(index = null) {
  editingIndex.value = index;
  editingSupplier.value = index === null ? createSupplier() : cloneSupplier(suppliers.value[index]);
  expandedModelNames.value = editingSupplier.value.models[0]?.localId
    ? [editingSupplier.value.models[0].localId]
    : [];
  nextTick(() => supplierFormRef.value?.restoreValidation());
}

function closeEditor() {
  editingIndex.value = null;
  editingSupplier.value = null;
  expandedModelNames.value = [];
}

function updateEditorVisibility(visible) {
  if (!visible && !saving.value) closeEditor();
}

function openAddModelDialog() {
  if (!editingSupplier.value || modelQuery.value.loading) return;
  addModelMethodOpen.value = true;
}

function addCustomModel() {
  if (!editingSupplier.value) return;
  const model = createModel();
  editingSupplier.value.models.push(model);
  expandedModelNames.value = [model.localId];
}

function chooseAddModelMethod(method) {
  addModelMethodOpen.value = false;
  if (method === "query") openModelQuery();
  else addCustomModel();
}

function copyModel(model) {
  if (!editingSupplier.value) return;
  const usedNames = new Set(editingSupplier.value.models
    .filter((item) => item !== model)
    .map((item) => item.name.trim().toLocaleLowerCase()));
  const copiedModel = {
    ...model,
    localId: createId(),
    name: uniqueModelName(`${model.name} copy`, usedNames),
    apiKeyVisible: false,
    params: normalizeModelParams(model.params),
  };
  editingSupplier.value.models.push(copiedModel);
  expandedModelNames.value = [copiedModel.localId];
}

function removeModel(index) {
  if (!editingSupplier.value) return;
  const removedModel = editingSupplier.value.models[index];
  editingSupplier.value.models.splice(index, 1);
  if (expandedModelNames.value[0] === removedModel.localId) {
    const nextModel = editingSupplier.value.models[Math.min(index, editingSupplier.value.models.length - 1)];
    expandedModelNames.value = nextModel ? [nextModel.localId] : [];
  }
}

function changeParamType(param, valueType) {
  param.valueType = valueType;
  param.value = normalizeParamValue(null, valueType);
  param.options = normalizeParamOptions(param.options, valueType);
}

function updateParamOptions(param, options) {
  const normalized = [];
  let hasEmptyOption = false;
  for (const option of options || []) {
    const value = String(option ?? "").trim();
    if (!value) hasEmptyOption = true;
    else if (!normalized.includes(value)) normalized.push(value);
  }
  if (hasEmptyOption) normalized.push("");
  param.options = normalized;
  const validOptions = normalized.filter(Boolean);
  if (validOptions.length && !validOptions.includes(param.value)) param.value = validOptions[0];
}

function addParamOption(param) {
  if (param.options[param.options.length - 1] !== "") param.options.push("");
}

function updateParamOption(param, index, value) {
  const options = [...param.options];
  options[index] = value;
  updateParamOptions(param, options);
}

function removeParamOption(param, index) {
  const options = [...param.options];
  options.splice(index, 1);
  updateParamOptions(param, options);
}

function changeFunctionType(model, functionType) {
  model.functionType = functionType;
  if (functionType === "image") model.apiFormat = "images";
  else if (model.apiFormat === "images" || model.apiFormat === "imagesApi") model.apiFormat = "chatCompletions";
}

function removeParam(model, index) {
  model.params.splice(index, 1);
}

function validateModelParams(supplier) {
  for (const model of supplier.models) {
    const keys = new Set();
    for (const param of model.params) {
      const key = param.key.trim();
      if (!key) return copy.value.paramKeyRequired;
      if (keys.has(key)) return copy.value.paramKeyDuplicate;
      keys.add(key);
      if (!parameterTypes.includes(param.valueType)) return copy.value.paramTypeInvalid;
      if (!String(param.description || "").trim()) return copy.value.paramDescriptionRequired;
      if (param.valueType === "number" && (param.value === null || !Number.isFinite(Number(param.value)))) {
        return copy.value.paramNumberInvalid;
      }
      if (param.valueType === "string" && !String(param.value || "").trim()) return copy.value.paramValueRequired;
      if (param.valueType === "string" && param.options.length && !param.options.includes(param.value)) {
        return copy.value.paramOptionInvalid;
      }
    }
  }
  return null;
}

function normalizeSupplier(supplier) {
  return {
    name: supplier.name.trim(),
    config: {
      officialAddress: supplier.config.officialAddress.trim(),
      baseURL: supplier.config.baseURL.trim(),
    },
    models: supplier.models.map((model) => ({
      id: model.id.trim(),
      name: model.name.trim(),
      apiFormat: model.apiFormat,
      functionType: model.functionType,
      requestPath: model.requestPath.trim(),
      apiKey: model.apiKey.trim(),
      params: normalizeModelParams(model.params),
    })),
  };
}

async function saveDraft() {
  try {
    await supplierFormRef.value?.validate();
  } catch {
    return;
  }
  const paramsError = validateModelParams(editingSupplier.value);
  if (paramsError) {
    message.error(paramsError, { duration: 2000 });
    return;
  }
  const normalized = normalizeSupplier(editingSupplier.value);
  const nextSuppliers = [...suppliers.value];
  let nextActiveModelId = activeModelId.value;
  if (editingIndex.value === null) {
    nextSuppliers.push(normalized);
    if (!nextActiveModelId && normalized.models.length) {
      nextActiveModelId = modelConfigId(normalized.name, normalized.models[0].name);
    }
  } else {
    const previous = nextSuppliers[editingIndex.value];
    if (nextActiveModelId?.startsWith(`${previous.name}::`)) {
      const activeName = nextActiveModelId.slice(previous.name.length + 2);
      const activeModel = normalized.models.find((model) => model.name === activeName) || normalized.models[0];
      nextActiveModelId = activeModel ? modelConfigId(normalized.name, activeModel.name) : null;
    }
    nextSuppliers[editingIndex.value] = normalized;
  }
  const saved = await persistSettings(nextSuppliers, nextActiveModelId);
  if (saved) closeEditor();
}

function removeSupplier(index) {
  suppliers.value.splice(index, 1);
  const first = suppliers.value[0];
  activeModelId.value = first?.models[0] ? modelConfigId(first.name, first.models[0].name) : null;
  persistSettings();
}

function persistSettings(nextSuppliers = suppliers.value, nextActiveModelId = activeModelId.value) {
  return saveSettings({
    theme: theme.value,
    language: locale.value,
    autostart: autostart.value,
    activeModelId: nextActiveModelId,
    suppliers: nextSuppliers,
  });
}

async function saveSettings(nextSettings) {
  saving.value = true;
  try {
    const result = await invoke("save_settings", { settings: nextSettings });
    theme.value = result.theme;
    locale.value = result.language;
    autostart.value = result.autostart;
    activeModelId.value = result.activeModelId;
    suppliers.value = result.suppliers.map(cloneSupplier);
    message.success(copy.value.saved);
    return true;
  } catch (saveError) {
    message.error(String(saveError), { duration: 2000 });
    return false;
  } finally {
    saving.value = false;
  }
}

async function clearData() {
  if (saving.value) return;
  try {
    await invoke("clear_app_data");
    theme.value = "system";
    locale.value = "zh-CN";
    autostart.value = false;
    activeModelId.value = null;
    suppliers.value = [];
    closeEditor();
    message.success(copy.value.saved);
  } catch (clearError) {
    message.error(String(clearError), { duration: 2000 });
  }
}

async function loadSettings() {
  loading.value = true;
  try {
    const result = await invoke("load_settings");
    theme.value = result.theme;
    locale.value = result.language;
    autostart.value = result.autostart;
    activeModelId.value = result.activeModelId;
    suppliers.value = result.suppliers.map(cloneSupplier);
  } catch (loadError) {
    message.error(`${copy.value.settingsLoadFailed}：${String(loadError)}`, { duration: 2000 });
  } finally {
    loading.value = false;
  }
}

function updateGeneralSettings() {
  persistSettings();
}

onMounted(loadSettings);
</script>

<template>
  <n-layout class="settings-page" :class="`theme-${theme}`">
    <n-layout-content content-style="padding: 28px 32px; max-width: 1120px; margin: 0 auto; width: 100%;">
      <n-page-header :title="copy.settingsTitle" :subtitle="copy.general" @back="router.push('/')">
        <template #back>
          <n-icon><ChevronLeftIcon /></n-icon>
        </template>
        <template #extra>
          <n-button quaternary @click="router.push('/')">{{ copy.back }}</n-button>
        </template>
      </n-page-header>

      <n-card class="settings-general-card" :title="copy.appearance" :bordered="false">
        <n-space vertical size="large">
          <n-space align="center" justify="space-between">
            <span>{{ copy.theme }}</span>
            <n-select v-model:value="theme" :options="[{ label: copy.systemTheme, value: 'system' }, { label: copy.lightTheme, value: 'light' }, { label: copy.darkTheme, value: 'dark' }]" style="width: 180px" @update:value="updateGeneralSettings" />
          </n-space>
          <n-space align="center" justify="space-between">
            <span>{{ copy.language }}</span>
            <n-select v-model:value="locale" :options="[{ label: copy.chinese, value: 'zh-CN' }, { label: copy.english, value: 'en-US' }]" style="width: 180px" @update:value="updateGeneralSettings" />
          </n-space>
          <n-space align="center" justify="space-between">
            <span>{{ copy.autostart }}</span>
            <n-switch v-model:value="autostart" @update:value="updateGeneralSettings" />
          </n-space>
        </n-space>
      </n-card>
      <div class="supplier-heading">
        <h2>{{ copy.modelSettings }}</h2>
        <p>{{ copy.modelDescription }}</p>
      </div>
      <n-space justify="end" class="supplier-actions">
        <n-button type="primary" @click="openSupplier()">
          <template #icon><n-icon><PlusIcon /></n-icon></template>
          {{ copy.addSupplier }}
        </n-button>
      </n-space>

      <n-empty v-if="!loading && suppliers.length === 0" :description="copy.noSuppliers" />
      <n-space v-else vertical size="large">
        <n-card v-for="(supplier, index) in suppliers" :key="supplier.name" :title="supplier.name" size="large">
          <template #header-extra>
            <n-space>
              <n-button secondary @click="openSupplier(index)">{{ copy.editSupplier }}</n-button>
              <n-popconfirm :positive-text="copy.confirm" :negative-text="copy.cancel" @positive-click="removeSupplier(index)">
                <template #trigger><n-button quaternary type="error">{{ copy.removeSupplier }}</n-button></template>
                {{ copy.removeSupplierConfirm }}
              </n-popconfirm>
            </n-space>
          </template>
          <n-space align="center">
            <n-tag type="info">{{ supplier.models.length }} {{ copy.modelCount }}</n-tag>
            <span>{{ supplier.config.baseURL }}</span>
            <span v-if="supplier.config.officialAddress">{{ supplier.config.officialAddress }}</span>
          </n-space>
        </n-card>
      </n-space>

      <n-modal
        v-if="editingSupplier"
        :show="Boolean(editingSupplier)"
        preset="card"
        :title="editingIndex === null ? copy.addSupplier : copy.editSupplier"
        :mask-closable="!saving"
        :closable="!saving"
        content-style="max-height: calc(100vh - 190px); overflow-y: auto;"
        :class="['supplier-editor-modal', themeClass]"
        @update:show="updateEditorVisibility"
      >
        <n-form
          ref="supplierFormRef"
          :model="editingSupplier"
          :rules="supplierRules"
          label-placement="top"
          show-require-mark
          require-mark-placement="right"
        >
          <n-form-item :label="copy.supplierName" path="name">
            <n-input v-model:value="editingSupplier.name" :placeholder="copy.supplierDefaultName" />
          </n-form-item>
          <n-grid :cols="2" :x-gap="16" responsive="screen">
            <n-grid-item>
              <n-form-item :label="copy.officialAddress" :show-require-mark="false">
                <n-input v-model:value="editingSupplier.config.officialAddress" :placeholder="copy.officialAddressPlaceholder" />
              </n-form-item>
            </n-grid-item>
            <n-grid-item>
              <n-form-item :label="copy.baseUrl" path="config.baseURL">
                <n-input v-model:value="editingSupplier.config.baseURL" :placeholder="copy.baseUrlPlaceholder" />
              </n-form-item>
            </n-grid-item>
          </n-grid>
          <n-divider>{{ copy.supplierModels }}</n-divider>
          <n-space vertical size="large">
            <n-collapse v-model:expanded-names="expandedModelNames" accordion>
              <n-collapse-item v-for="(model, modelIndex) in editingSupplier.models" :key="model.localId" :name="model.localId">
                <template #header>
                  <strong>{{ modelIndex + 1 }} · {{ model.name || copy.unnamedModel }}</strong>
                </template>
                <template #header-extra>
                  <n-space @click.stop>
                    <n-button text type="primary" @click.stop="copyModel(model)">{{ copy.copyModel }}</n-button>
                    <n-button text type="error" @click.stop="removeModel(modelIndex)">{{ copy.removeModel }}</n-button>
                  </n-space>
                </template>
                <n-grid :cols="2" :x-gap="16" responsive="screen">
                    <n-grid-item>
                      <n-form-item :label="copy.modelName" :path="`models.${modelIndex}.name`">
                        <n-input v-model:value="model.name" :placeholder="copy.modelNamePlaceholder" />
                      </n-form-item>
                    </n-grid-item>
                    <n-grid-item>
                      <n-form-item :label="copy.modelId" :path="`models.${modelIndex}.id`">
                        <n-input v-model:value="model.id" :placeholder="copy.modelIdPlaceholder" />
                      </n-form-item>
                    </n-grid-item>
                    <n-grid-item>
                      <n-form-item :label="copy.apiFormat" :path="`models.${modelIndex}.apiFormat`">
                        <div class="api-format-field">
                          <n-select v-model:value="model.apiFormat" :options="apiFormatOptions" />
                          <div class="api-format-endpoint-hint">
                            <span>{{ model.requestPath.trim() ? copy.requestPathCustomHint : copy.apiFormatEndpointHint }}：</span>
                            <code>{{ model.requestPath.trim() || apiFormatEndpoint(model.apiFormat) }}</code>
                          </div>
                        </div>
                      </n-form-item>
                    </n-grid-item>
                    <n-grid-item>
                      <n-form-item :label="copy.requestPath" :path="`models.${modelIndex}.requestPath`" :show-require-mark="false">
                        <n-input v-model:value="model.requestPath" :placeholder="copy.requestPathPlaceholder" />
                      </n-form-item>
                    </n-grid-item>
                    <n-grid-item>
                      <n-form-item :label="copy.apiKey" :path="`models.${modelIndex}.apiKey`">
                        <div class="model-api-key-field">
                          <n-input v-model:value="model.apiKey" :type="model.apiKeyVisible ? 'text' : 'password'" :placeholder="copy.apiKeyPlaceholder">
                            <template #suffix>
                              <n-button text size="small" @click="model.apiKeyVisible = !model.apiKeyVisible">{{ model.apiKeyVisible ? copy.hideApiKey : copy.showApiKey }}</n-button>
                            </template>
                          </n-input>
                        </div>
                      </n-form-item>
                    </n-grid-item>
                    <n-grid-item>
                      <n-form-item :label="copy.functionType" :path="`models.${modelIndex}.functionType`">
                        <n-select v-model:value="model.functionType" :options="functionTypeOptions" @update:value="changeFunctionType(model, $event)" />
                      </n-form-item>
                    </n-grid-item>
                </n-grid>
                <div class="params-heading">
                  <span class="settings-muted">{{ copy.customParamsHint }}</span>
                </div>
                <n-table :bordered="false" :single-line="false" size="small" class="params-table">
                  <thead>
                    <tr>
                      <th>{{ copy.paramKey }}</th>
                      <th>{{ copy.paramDescription }}</th>
                      <th>{{ copy.paramType }}</th>
                      <th>{{ copy.paramValue }}</th>
                      <th>{{ copy.paramOptions }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-if="!model.params.length">
                      <td colspan="5">
                        <n-empty size="small" :description="copy.noCustomParams" />
                      </td>
                    </tr>
                    <tr v-for="(param, paramIndex) in model.params" :key="paramIndex">
                      <td><n-input v-model:value="param.key" size="small" :placeholder="copy.paramKeyPlaceholder" /></td>
                      <td><n-input v-model:value="param.description" size="small" :placeholder="copy.paramDescriptionPlaceholder" /></td>
                      <td><n-select v-model:value="param.valueType" size="small" :options="parameterTypeOptions" @update:value="changeParamType(param, $event)" /></td>
                      <td>
                        <div class="param-value-cell">
                          <n-input v-if="param.valueType === 'string' && !param.options.some(Boolean)" v-model:value="param.value" size="small" :placeholder="copy.paramValuePlaceholder" />
                          <n-select v-else-if="param.valueType === 'string'" v-model:value="param.value" :options="param.options.filter(Boolean).map((option) => ({ label: option, value: option }))" size="small" />
                          <n-switch v-else-if="param.valueType === 'boolean'" v-model:value="param.value" />
                          <n-input-number v-else v-model:value="param.value" size="small" :placeholder="copy.paramValuePlaceholder" style="width: 100%" />
                          <n-button text type="error" size="small" @click="removeParam(model, paramIndex)">{{ copy.removeParam }}</n-button>
                        </div>
                      </td>
                      <td>
                        <div v-if="param.valueType === 'string'" class="param-options-cell">
                          <div v-for="(option, optionIndex) in param.options" :key="optionIndex" class="param-option-row">
                            <n-input
                              :value="option"
                              size="small"
                              :placeholder="copy.paramOptionPlaceholder"
                              @update:value="updateParamOption(param, optionIndex, $event)"
                            />
                            <n-button text type="error" size="small" :title="copy.removeParamOption" :aria-label="copy.removeParamOption" @click="removeParamOption(param, optionIndex)">
                              <template #icon><n-icon><TrashIcon /></n-icon></template>
                            </n-button>
                          </div>
                          <n-button text type="primary" size="small" :title="copy.addParamOption" :aria-label="copy.addParamOption" @click="addParamOption(param)">
                            <template #icon><n-icon><PlusIcon /></n-icon></template>
                          </n-button>
                        </div>
                        <span v-else class="settings-muted">{{ copy.paramOptionsUnavailable }}</span>
                      </td>
                    </tr>
                  </tbody>
                  <tfoot>
                    <tr class="params-table-footer">
                      <td colspan="5">
                        <n-button size="small" secondary @click="model.params.push(createParam())">
                          <template #icon><n-icon><PlusIcon /></n-icon></template>
                          {{ copy.addParam }}
                        </n-button>
                      </td>
                    </tr>
                  </tfoot>
                </n-table>
              </n-collapse-item>
            </n-collapse>
            <n-space>
              <n-button dashed @click="openAddModelDialog">
                <template #icon><n-icon><PlusIcon /></n-icon></template>
                {{ copy.addModel }}
              </n-button>
            </n-space>
          </n-space>
          <n-space justify="end" class="settings-save-actions">
            <n-button @click="closeEditor">{{ copy.cancel }}</n-button>
            <n-button type="primary" :loading="saving" @click="saveDraft">{{ copy.modelSave }}</n-button>
          </n-space>
        </n-form>
      </n-modal>

      <n-modal
        :show="addModelMethodOpen"
        preset="card"
        :title="copy.addModel"
        :mask-closable="true"
        :closable="true"
        class="add-model-method-modal"
        @update:show="addModelMethodOpen = $event"
      >
        <n-space vertical size="medium">
          <div
            class="add-model-method-option"
            role="button"
            tabindex="0"
            @click="chooseAddModelMethod('query')"
            @keydown.enter="chooseAddModelMethod('query')"
            @keydown.space.prevent="chooseAddModelMethod('query')"
          >
            <span class="add-model-method-content">
              <strong>{{ copy.addModelByApiKey }}</strong>
              <span class="settings-muted">{{ copy.addModelByApiKeyDescription }}</span>
            </span>
          </div>
          <div
            class="add-model-method-option"
            role="button"
            tabindex="0"
            @click="chooseAddModelMethod('custom')"
            @keydown.enter="chooseAddModelMethod('custom')"
            @keydown.space.prevent="chooseAddModelMethod('custom')"
          >
            <span class="add-model-method-content">
              <strong>{{ copy.addModelCustom }}</strong>
              <span class="settings-muted">{{ copy.addModelCustomDescription }}</span>
            </span>
          </div>
        </n-space>
      </n-modal>

      <n-modal
        v-if="modelQuery.open"
        :show="modelQuery.open"
        preset="card"
        :title="copy.modelListTitle"
        :mask-closable="false"
        :closable="true"
        class="model-list-modal"
        @update:show="(visible) => { if (!visible) closeModelQuery(); }"
      >
        <n-space vertical size="large">
          <p class="settings-muted model-list-description">{{ copy.modelListDescription }}</p>
          <n-form-item :label="copy.apiKey" :show-require-mark="false">
            <n-input v-model:value="modelQuery.apiKey" type="password" :placeholder="copy.apiKeyPlaceholder" />
          </n-form-item>
          <n-button type="primary" block :loading="modelQuery.loading" @click="queryProviderModels(modelQuery.targetModel)">
            {{ copy.queryModels }}
          </n-button>
          <template v-if="queriedModelCount">
          <n-input
            v-model:value="modelQuery.search"
            clearable
            :placeholder="copy.modelListSearchPlaceholder"
          />
          <n-space justify="space-between" align="center">
            <n-checkbox
              :checked="allQueriedModelsSelected"
              :indeterminate="someQueriedModelsSelected"
              @update:checked="toggleAllQueriedModels"
            >
              {{ copy.selectAllModels }}
            </n-checkbox>
            <span class="settings-muted model-list-count">{{ selectedQueriedModelCount }} / {{ queriedModelCount }}</span>
          </n-space>
          <div class="model-list-options">
            <label v-for="item in filteredQueriedModels" :key="item.id" class="model-list-option">
              <n-checkbox
                :checked="modelQuery.selectedIds.includes(item.id)"
                @update:checked="toggleQueriedModel(item.id, $event)"
              />
              <span class="model-list-option-main">
                <strong>{{ item.name }}</strong>
                <code>{{ item.id }}</code>
              </span>
              <span v-if="item.ownedBy" class="model-list-option-owner">{{ item.ownedBy }}</span>
            </label>
            <n-empty v-if="!visibleQueriedModelCount" size="small" :description="copy.modelListSearchEmpty" />
          </div>
          <n-space justify="end">
            <n-button @click="closeModelQuery">{{ copy.cancel }}</n-button>
            <n-button type="primary" :disabled="selectedQueriedModelCount === 0" @click="importQueriedModels">
              {{ copy.importSelectedModels }}
            </n-button>
          </n-space>
          </template>
        </n-space>
      </n-modal>

      <n-divider />
      <n-card :bordered="false">
        <n-popconfirm :positive-text="copy.confirm" :negative-text="copy.cancel" @positive-click="clearData">
          <template #trigger><n-button type="error" secondary>{{ copy.clearData }}</n-button></template>
          {{ copy.clearDataConfirm }}
        </n-popconfirm>
        <p class="settings-muted">{{ copy.clearDataDescription }}</p>
      </n-card>
    </n-layout-content>
  </n-layout>
</template>

<style scoped>
.settings-page {
  min-height: 100vh;
  background: var(--app-bg);
  color: var(--text);
}

.settings-page :deep(.n-layout-scroll-container) {
  min-height: 100vh;
}

.settings-page :deep(.n-page-header) {
  margin-bottom: 24px;
}

.settings-page :deep(.n-page-header__title) {
  color: var(--text);
}

.settings-page :deep(.n-page-header__subtitle) {
  color: var(--text-muted);
}

.settings-general-card {
  margin-bottom: 24px;
}

.settings-general-card :deep(.n-card__content) {
  padding-top: 6px;
}

.supplier-heading {
  margin-bottom: 20px;
}

.supplier-heading h2 {
  margin: 0;
  color: var(--text);
  font-size: 20px;
}

.supplier-heading p {
  margin: 6px 0 0;
  color: var(--text-muted);
  font-size: 13px;
}

.supplier-editor-modal :deep(.n-card) {
  width: 100%;
  max-width: 100%;
  border-color: var(--border);
}

:global(.supplier-editor-modal) {
  width: min(1120px, calc(100vw - 48px)) !important;
  max-width: calc(100vw - 48px) !important;
  box-sizing: border-box;
}

.supplier-editor-modal :deep(.n-card__content) {
  min-width: 0;
}

.supplier-editor-modal :deep(.n-alert) {
  margin-bottom: 16px;
}

.supplier-editor-modal :deep(.n-card__header) {
  color: var(--text);
}

.supplier-editor-modal :deep(.n-form-item-label__text),
.supplier-editor-modal :deep(.n-divider__title) {
  color: var(--text-secondary);
}

.model-api-key-field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 100%;
}

:global(.model-list-modal) {
  width: min(680px, calc(100vw - 48px)) !important;
  max-width: calc(100vw - 48px) !important;
}

:global(.add-model-method-modal) {
  width: min(520px, calc(100vw - 32px)) !important;
  max-width: calc(100vw - 32px) !important;
}

.add-model-method-content {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  width: 100%;
  gap: 5px;
  padding: 4px 2px;
  text-align: left;
  white-space: normal;
}

.add-model-method-option {
  display: block;
  width: 100%;
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-subtle);
  cursor: pointer;
  transition: border-color 0.15s ease, background-color 0.15s ease;
}

.add-model-method-option:hover,
.add-model-method-option:focus-visible {
  border-color: var(--accent);
  background: var(--accent-soft);
  outline: none;
}

.add-model-method-content strong {
  color: var(--text);
  font-size: 15px;
}

.add-model-method-content .settings-muted {
  line-height: 1.5;
  white-space: normal;
}

.model-list-description {
  margin: 0;
}

.model-list-count {
  margin: 0;
}

.model-list-options {
  max-height: min(440px, 50vh);
  overflow-y: auto;
  border: 1px solid var(--border-subtle);
  border-radius: 6px;
}

.model-list-option {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 58px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-subtle);
  cursor: pointer;
}

.model-list-option:last-child {
  border-bottom: 0;
}

.model-list-option:hover {
  background: var(--surface-subtle);
}

.model-list-option-main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  gap: 3px;
}

.model-list-option-main strong,
.model-list-option-main code {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.model-list-option-main strong {
  color: var(--text);
  font-size: 14px;
}

.model-list-option-main code,
.model-list-option-owner {
  color: var(--text-muted);
  font-size: 12px;
}

.model-list-option-owner {
  flex: 0 0 auto;
}

.settings-save-actions {
  margin-top: 22px;
}

.settings-muted {
  margin: 12px 0 0;
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1.6;
}

.protocol-option {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 16px;
  width: 100%;
}

.protocol-option-endpoint {
  color: var(--text-muted);
  font-size: 12px;
}

.api-format-field {
  display: flex;
  flex-direction: column;
  width: 100%;
  min-width: 0;
}

.api-format-field :deep(.n-select) {
  width: 100%;
}

.api-format-endpoint-hint {
  display: flex;
  align-items: baseline;
  gap: 6px;
  margin-top: 6px;
  color: var(--text-muted);
  font-size: 12px;
  line-height: 1.5;
}

.api-format-endpoint-hint code {
  color: var(--text-secondary);
  font-family: inherit;
}

.params-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.params-heading .settings-muted {
  margin: 0;
}

.params-table {
  overflow-x: auto;
}

.params-table :deep(th),
.params-table :deep(td) {
  min-width: 140px;
  vertical-align: middle;
}

.params-table :deep(th:first-child),
.params-table :deep(td:first-child) {
  min-width: 170px;
}

.params-table :deep(th:nth-child(2)),
.params-table :deep(td:nth-child(2)) {
  min-width: 220px;
}

.params-table-footer :deep(td) {
  padding: 10px 12px;
  background: transparent;
}

.param-value-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 190px;
}

.param-value-cell .n-input,
.param-value-cell .n-input-number {
  min-width: 0;
  flex: 1;
}

.param-options-cell {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  min-width: 190px;
}

.param-option-row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
}

.param-option-row .n-input {
  min-width: 0;
  flex: 1;
}

.param-option-row .n-button,
.param-options-cell > .n-button {
  flex: 0 0 auto;
}

@media (max-width: 720px) {
  .settings-page :deep(.n-layout-content) {
    padding: 20px 14px !important;
  }

  .supplier-heading {
    align-items: flex-start !important;
    flex-direction: column;
    gap: 14px;
  }

  .supplier-editor-modal :deep(.n-card) {
    width: 100%;
  }

  :global(.supplier-editor-modal) {
    width: calc(100vw - 24px) !important;
    max-width: calc(100vw - 24px) !important;
  }
}
</style>
