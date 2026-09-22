import { createRouter, createWebHashHistory } from "vue-router";
import HomeView from "./views/HomeView.vue";
import SettingsView from "./views/SettingsView.vue";
import WorkspaceView from "./views/WorkspaceView.vue";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "home", component: HomeView },
    { path: "/settings", name: "settings", component: SettingsView },
    { path: "/workspace", name: "workspace", component: WorkspaceView },
  ],
});

export default router;
