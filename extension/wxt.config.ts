import { defineConfig } from "wxt";

const icons = {
  "16": "images/kdeconnect-16.png",
  "22": "images/kdeconnect-22.png",
  "24": "images/kdeconnect-24.png",
  "32": "images/kdeconnect-32.png",
  "48": "images/kdeconnect-48.png",
  "64": "images/kdeconnect-64.png",
  "128": "images/kdeconnect-128.png",
  "256": "images/kdeconnect-256.png",
} as const;

export default defineConfig({
  srcDir: "src",
  publicDir: "public",
  imports: false,
  modules: ["@wxt-dev/i18n/module"],
  manifest: ({ manifestVersion }) => ({
    name: "__MSG_kdeconnect_name__",
    short_name: "kdeconnect-browser-reborn",
    description: "__MSG_kdeconnect_description__",
    default_locale: "en",
    homepage_url: "https://github.com/MuntasirSZN/kdeconnect-browser-reborn",
    permissions: ["nativeMessaging", "storage", "contextMenus", "activeTab", "tabs"],
    host_permissions: ["*://*/*"],
    action: {
      default_title: "__MSG_kdeconnect_root__",
      default_popup: "popup.html",
      default_icon: icons,
    },
    commands: {
      [manifestVersion === 2 ? "_execute_browser_action" : "_execute_action"]: {
        suggested_key: { default: "Alt+C" },
      },
    },
    browser_specific_settings: {
      gecko: {
        id: "kdeconnect-browser-reborn@muntasirszn.github.io",
        strict_min_version: "140.0",
        data_collection_permissions: { required: ["none"] },
      },
    },
    icons,
  }),
});
