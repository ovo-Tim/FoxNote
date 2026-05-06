import "vuetify/styles";
import { createVuetify } from "vuetify";
import * as components from "vuetify/components";
import * as directives from "vuetify/directives";
import { aliases, mdi } from "vuetify/iconsets/mdi";

const vuetify = createVuetify({
  components,
  directives,
  icons: {
    defaultSet: "mdi",
    aliases,
    sets: {
      mdi,
    },
  },
  theme: {
    defaultTheme: "foxnote",
    themes: {
      foxnote: {
        dark: true,
        colors: {
          background: "#1f2228",
          surface: "#252932",
          primary: "#70a5ff",
          secondary: "#8f9cb5",
          error: "#ff7d7d",
          info: "#76c7ff",
          success: "#76d487",
          warning: "#ffbc6b",
        },
      },
    },
  },
});

export default vuetify;
