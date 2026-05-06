import { createApp } from "vue";
import App from "./App.vue";
import "@mdi/font/css/materialdesignicons.css";
import vuetify from "./plugins/vuetify";
import "./styles/theme.css";

createApp(App).use(vuetify).mount("#app");
